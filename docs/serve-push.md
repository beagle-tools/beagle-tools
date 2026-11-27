# Push notifications: Mac → Apple → phone (design)

> **Status: approved; being built** in the order in [Build order](#build-order).
> Steps 1 to 3 are in: the event spool, phones and settings, the
> follower (`scout/tui/src/events.rs`, `beagle/serve/src/push/`), the
> APNs sender with `push setup` / `push test` (`push/apns.rs`, `jwt.rs`,
> `cmd.rs`), and the app (`utils/swift/Sources/BeaglePush`,
> `NotificationSettingsSection`, `ios/Beagle/AppDelegate.swift`). Without
> `[push]`, serve logs the notifications it would send.
> The device endpoints are also listed in [`serve-api.md`](serve-api.md).

`beagle scout` already shows a macOS banner when an agent changes a status
(`scout/tui/src/notify.rs`): a task is blocked (with a sound), a project
starts or finishes, a working task goes quiet. This design sends the same
events to the Beagle iPhone app as real push notifications. They arrive while
the app is closed, can be switched on or off per tool and per event in the
app, and open the task when tapped.

iOS shows a notification for a closed app only if it arrives through Apple's
Push Notification service (APNs). The Mac can call APNs directly with a
token-auth key (`.p8`) from the Apple developer account. No third-party
server and no relay are involved: the Mac is the provider.

## The flow

```text
 beagle scout task block …            beagle serve (Mac)                    Apple            iPhone
 ─────────────────────────            ─────────────────                     ─────            ──────
 status changes                       follower thread
   │                                    tails the spool ──▶ event
   ├─▶ macOS banner (as today)                               │ for each registered phone:
   └─▶ append one JSON line ──────▶   events/scout.jsonl     │ its settings allow this event?
       (the event spool)                                     ▼
                                      sender thread ──HTTP/2 + JWT──▶ APNs ──▶ banner, sound,
                                        retries, drops dead tokens             tap → the task
 Beagle app ──PUT /v1/devices/{id}──▶ devices.json (0600)
            ──PUT …/notifications──▶  per-tool settings
```

1. **One event source.** The place in scout that builds the macOS banner
   today (`app/status.rs` `notify_event`, and `autopause::sweep`) also
   appends the event to a spool file. The banner and the push share one
   moment and one wording.
2. **`beagle serve` follows the spool.** It matches each new event against
   every registered phone's settings and hands matches to a sender.
3. **The sender calls APNs**, signed with the developer key, and handles
   retries and dead tokens.
4. **The phone** registers its APNs device token and its settings with
   `beagle serve`, over the same tailnet + bearer token as the read API.

## 1. The event spool

`$XDG_STATE_HOME/beagle/events/<tool>.jsonl` (so
`~/.local/state/beagle/events/scout.jsonl`), one JSON object per line:

```json
{"v":1,"id":"5f0c9a2e-…","time":"2026-11-25T18:51:21Z","tool":"scout","kind":"blocked",
 "project":"rate-limit-the-api","project_title":"Rate-limit the public API",
 "task":"03-open-pr","task_title":"Open the PR",
 "title":"Scout · rate-limit-the-api","subtitle":"■ Needs you: 03-open-pr · Open the PR",
 "body":"Push branch fix-retry and open a PR?",
 "link":"beagle://scout/projects/rate-limit-the-api/tasks/03-open-pr"}
```

- **`kind`** is one of scout's notification events, in the names
  `scout.toml` already uses: `blocked`, `project_started`, `project_done`,
  `task_done`, `task_started`, `task_paused`. `task`/`task_title` are `null`
  for project events.
- **The text is rendered once, by scout.** `title`/`subtitle`/`body` are
  exactly the macOS banner's (`notify::build`, without the config filter).
  `beagle serve` doesn't need scout's wording rules, and the phone and the
  Mac say the same thing.
- **Every event is written**, whatever `scout.toml`'s `notify` and
  `[notify_events]` say: those switches are for the Mac's banners. The phone
  has its own settings (section 2). `BEAGLE_EVENTS=0` turns the spool off
  (tests). `SCOUT_NOTIFY=0` still only silences banners.
- **Append-only, never fails the command.** It is one `O_APPEND` write of
  one line, under 4 KiB (`body` is cut to 1,000 characters), so concurrent
  agents don't interleave. Any error is ignored, like a banner that fails to
  show.
- **Bounded.** When the file passes 1 MiB, the writer renames it to
  `<tool>.jsonl.1` (replacing the old one) before appending.
- Why a state-dir file and not the scout store: the store is often a git
  repo the user pushes, and events are machine-local noise. A file is
  something the CLI can write without a daemon, and `beagle serve` can
  follow it without linking into every command.

**Following it.** A `beagle serve` thread polls the spool every 2 seconds.
It keeps a cursor (file identity + byte offset) in
`~/.local/state/beagle/serve/push-cursor.json`.

- It reads only whole lines and skips any it can't parse, with one log line.
- On rotation it finishes the renamed file first.
- On first start, or after a cursor loss, it starts at the end of the file,
  so old events are never replayed.
- After downtime it delivers missed events up to **10 minutes** old and
  drops the rest. A Mac that slept overnight doesn't wake the phone with a
  burst of stale news.

## 2. Phones and their settings: the first write endpoints

The read API stays exactly as it is. These are the only non-`GET` routes,
under the same rules: tailnet only, HTTPS, `Authorization: Bearer <token>`,
the same error body.

| Method + path | What it does |
|---|---|
| `PUT /v1/devices/{device}` | Register or refresh a phone (idempotent) |
| `DELETE /v1/devices/{device}` | Forget a phone (the app calls it on Unpair) |
| `GET /v1/devices/{device}/notifications` | Its settings, plus the catalog of tools and events |
| `PUT /v1/devices/{device}/notifications` | Replace its settings |
| `POST /v1/devices/{device}/test` | Send one test push to it now |

`{device}` is an install id the app makes once: a random UUID kept in its
Keychain. It stays stable across APNs token changes and app updates.

**Register** (the app calls it on every launch once it has an APNs token,
since tokens can change):

```http
PUT /v1/devices/9b1d…-uuid
Content-Type: application/json

{ "apns_token": "a1b2c3…hex…", "environment": "development",
  "topic": "dev.beagletools.beagle", "name": "iPhone" }
```

- `topic` is the app's bundle ID, which APNs needs as `apns-topic`. The
  app sends its own (`Bundle.main.bundleIdentifier`), so a developer
  building under a local bundle-ID override needs no server config.
- `environment` is `development` for Xcode installs (the APNs sandbox) or
  `production` for TestFlight/App Store builds. The app sends a build-time
  value. If APNs answers `BadDeviceToken`, the sender tries the other host
  once and stores the one that worked (section 3).
- Answers `200` with the device's settings document (below), or `201` the
  first time.

**Settings** (`GET` answers this; `PUT` takes the `settings` part):

```json
{
  "settings": {
    "scout": { "enabled": true,
               "events": { "blocked": true, "project_started": true, "project_done": true,
                           "task_done": false, "task_started": false, "task_paused": true } }
  },
  "catalog": [
    { "tool": "scout", "name": "Scout", "events": [
      { "kind": "blocked", "label": "A task is blocked (needs you)", "default": true, "sound": true },
      { "kind": "project_started", "label": "A project starts", "default": true, "sound": false },
      { "kind": "project_done", "label": "A project is done", "default": true, "sound": false },
      { "kind": "task_done", "label": "Every finished task", "default": false, "sound": false },
      { "kind": "task_started", "label": "Every task an agent picks up (noisy)", "default": false, "sound": false },
      { "kind": "task_paused", "label": "A working task went quiet", "default": true, "sound": false } ] }
  ]
}
```

- **Defaults mirror the tool's own.** A new phone gets scout's
  `NotifyEvents::default()`: blocked, project started, project done and
  task paused on; task done and task started off. The catalog's labels
  and defaults come from the tool's crate, so the app and the Mac never
  disagree on what an event is called.
- The app draws its settings screen from `catalog`, so a new tool's
  events show up without an app change to the list.
- `PUT …/notifications` replaces the whole `settings` object. An unknown
  tool or kind, or a non-boolean, is `400`. A tool missing from the object
  keeps its defaults.

**Storage.** `~/.local/state/beagle/serve/devices.json`, mode `0600`,
written atomically:

```json
{ "devices": { "9b1d…": { "apns_token": "…", "environment": "development",
  "topic": "dev.beagletools.beagle", "name": "iPhone", "registered": "…",
  "last_seen": "…", "settings": { … } } } }
```

`beagle serve pair --rotate`, which already logs every phone out, also
clears it.

**Validation**

- Request bodies are JSON (`Content-Type: application/json`) with
  `Content-Length` and at most 16 KiB. Chunked bodies are refused.
- Unknown fields are `400`.
- `apns_token` is 64–200 hex characters.
- `environment` is `development` or `production`.
- `topic` is a reverse-DNS id of at most 155 characters.
- `name` is at most 64 printable characters.
- At most 10 devices; registering an 11th is `409 too_many_devices`.

| New status | `code` | When |
|---|---|---|
| 201 | — | A device registered for the first time |
| 204 | — | `DELETE` done (also when it was already gone) |
| 404 | `not_found` | `GET`/`PUT …/notifications` or `test` for an unknown device |
| 409 | `too_many_devices` | Registering an 11th phone |
| 415 | `unsupported_media_type` | A body that isn't JSON |
| 503 | `push_not_configured` | `test` while `[push]` isn't set up |

The request log line stays as it is (never the body). It logs device ids,
never APNs tokens.

## 3. Sending through APNs

**Config.** A new table in `~/.config/beagle/serve.toml` (already `0600`):

```toml
[push]
team_id = "ABC123DEFG"                                   # your Apple developer Team ID
key_id  = "KEY1234567"                                    # the APNs key's Key ID
key     = "~/.config/beagle/serve/AuthKey_KEY1234567.p8"  # the key Apple let you download once
```

- No `[push]` table means push is off. The device endpoints still work, so
  the app can be set up first. `beagle serve status` and `beagle doctor` say
  whether push is configured.
- `beagle serve` refuses a `.p8` readable by group or others, naming the
  `chmod 600` to run.
- `beagle serve push setup --key <file> --key-id <id> --team-id <id>`
  checks the key parses, copies it to `~/.config/beagle/serve/` with mode
  0600, and writes the table.

**Auth.** The sender signs an ES256 JWT with `ring` (already in the lockfile
through rustls):

- header `{"alg":"ES256","kid":<key_id>}`
- claims `{"iss":<team_id>,"iat":<now>}`

It reuses one token for 50 minutes: APNs rejects tokens older than an hour
and throttles ones refreshed more often than every 20 minutes. It refreshes
early on `403 ExpiredProviderToken`.

**Transport.** APNs speaks only HTTP/2:

- host `api.sandbox.push.apple.com` (development) or `api.push.apple.com`
  (production), port 443
- request `POST /3/device/<apns_token>`

The sender uses `reqwest` (already used in the workspace) with its `http2`
feature, adding the `h2` crate, on a current-thread tokio runtime in its
own thread. That is the same pattern tandem's review TUI uses: the rest of
`beagle serve` stays synchronous. (Shelling out to macOS `curl --http2`
would add no crate, but it would put the JWT on a command line and make the
responses harder to test.)

**Request**

| Header | Value |
|---|---|
| `authorization` | `bearer <jwt>` |
| `apns-topic` | the device's `topic` |
| `apns-push-type` | `alert` |
| `apns-priority` | `10` for `blocked`, `5` otherwise |
| `apns-expiration` | now + 24 h for `blocked`, now + 1 h otherwise |
| `apns-id` | the event's `id` |

```json
{ "aps": { "alert": { "title": "Scout · rate-limit-the-api",
                      "subtitle": "■ Needs you: 03-open-pr · Open the PR",
                      "body": "Push branch fix-retry and open a PR?" },
           "thread-id": "scout-rate-limit-the-api",
           "sound": "default" },
  "beagle": { "tool": "scout", "kind": "blocked",
              "link": "beagle://scout/projects/rate-limit-the-api/tasks/03-open-pr" } }
```

- Title, subtitle and body are the macOS banner's, as in the spool.
- `thread-id` groups a project's notifications on the lock screen, as the
  banner's `group` does on the Mac.
- `sound` is present only for `blocked`, the same rule as the Mac's.
  `interruption-level` is left at its default. Time-sensitive alerts need
  an extra entitlement, which is not worth it in v1.
- The payload stays under APNs' 4 KiB limit: `body` is already capped, and
  the encoder cuts it further if a long title would push past 4 KiB.

**Responses**

| APNs answer | What the sender does |
|---|---|
| `200` | Done; log one line: time, tool, kind, device id, `apns-id` |
| `400 BadDeviceToken` | Try the other environment once. If that works, store it on the device; else drop the device's token (the app re-registers on next launch) |
| `410 Unregistered` | The app was deleted or notifications revoked: remove the device |
| `403 ExpiredProviderToken` | New JWT, retry once |
| `403 InvalidProviderToken` and other `403`s | Config problem: log it loudly (`beagle serve status` shows the last error), don't retry |
| `413`, other `400`s | Log and drop the event (a bug on our side) |
| `429`, `5xx`, connection errors | Retry after 1 s, 4 s and 16 s, then drop |

Sends are sequential from one queue, with one HTTP/2 connection kept open
while there is work. That is a handful of pushes a day for one user, so no
batching. Nothing logs an APNs token or the JWT; a device appears only by
its id.

**Test command:** `beagle serve push test [--device <id>]` sends "Beagle ·
test push" to every registered phone, or one, and prints APNs' answer for
each. `POST /v1/devices/{id}/test` does the same for the app's "Send a test
notification" button.

## 4. The app

- **Capability.** The Beagle target gets the Push Notifications
  capability: a committed `Beagle.entitlements` with
  `aps-environment = development`. Signing stays automatic with no team.
  Automatic signing turns Push on for the App ID when the profile is made.
  TestFlight/App Store archives are re-signed with `production`.
- **Asking.** Never at launch. Settings → Notifications shows "Turn on
  notifications". Tapping it calls
  `UNUserNotificationCenter.requestAuthorization([.alert, .sound])`. If
  the user said no before, it offers "Open iOS Settings" instead.
- **Registering.** When authorized, the app calls
  `registerForRemoteNotifications()` on every launch (through a
  `UIApplicationDelegateAdaptor`). The token callback sends
  `PUT /v1/devices/{install id}`. A failure shows on the Notifications
  screen and is retried next launch; it never blocks the app.
- **Settings screen.** One section per tool from `catalog`: a master toggle
  plus one toggle per event, labelled as the catalog says, mirroring
  `scout.toml`'s `[notify_events]`. Changes `PUT` the settings right away.
  There is also a "Send a test notification" button. Lives in BeagleUI
  (`NotificationSettingsSection`) next to `ServerSection`, so every tool's
  screens share it.
- **Tap → the task.** A `UNUserNotificationCenterDelegate` reads
  `beagle.link` and sends it through the same path as `onOpenURL`.
  - `beagle://scout/projects/<id>/tasks/<task>` sets the navigation stack
    to home → Scout → project → task. `…/projects/<id>` stops at the
    project.
  - The home screen's tool registry routes the link by its first segment,
    so each tool owns its own link shapes.
  - The app shows banners while it is in the foreground too.
- **Unpair** sends `DELETE /v1/devices/{id}` first (best effort), then
  forgets the token, so the Mac stops pushing to a phone that left.

## 5. Other tools later

Push isn't scout-specific. A tool joins by:

1. writing its events to `events/<tool>.jsonl` in the same shape (its own
   `kind` names, its own `link` shape), and
2. giving `beagle serve` its catalog entry (kinds, labels, defaults, which
   kinds play a sound) from its crate, as scout's comes from
   `NotifyEvents`.

Likely first ones:

- rca: an RCA reaches `final-review`, or its agent opened a PR.
- recap: a recap is ready.
- standup: today's report is written.
- tandem: a queued review finished.
- agent: a run failed.

The app needs no list change: the catalog drives the settings screen, and
the tool's module handles its `link`.

## One-time Apple setup (each developer)

1. **Create the key.** Go to developer.apple.com → Certificates, IDs &
   Profiles → Keys → **+**, tick **Apple Push Notifications service
   (APNs)** and continue. Download `AuthKey_<KEYID>.p8`; Apple lets you
   download it **once**. Note the **Key ID** (on the key) and your **Team
   ID** (Membership details).
2. **Give it to the Mac:**
   `beagle serve push setup --key ~/Downloads/AuthKey_<KEYID>.p8 --key-id <KEYID> --team-id <TEAMID>`,
   then delete the download. The key, key ID and team ID live only in
   `~/.config/beagle/` on the Mac, never in the repo.
3. **Rebuild the app from Xcode** with your local signing. Push is in the
   committed entitlements. If Xcode says the profile lacks Push (for
   example under your local bundle-ID override), open Signing &
   Capabilities, which offers to fix it, or enable Push Notifications for
   that App ID in the developer portal (Identifiers).
4. **On the phone:** Settings → Notifications → Turn on, then "Send a test
   notification". Or run `beagle serve push test` on the Mac.

## Keeping the key in a secret manager

Instead of step 2's key file, the key can live in any secret manager, with
only a copy in the macOS Keychain. Store three secrets in it:

| Name | Value |
|---|---|
| `BEAGLE_APNS_KEY` | the whole `.p8` file, `-----BEGIN PRIVATE KEY-----` to `-----END PRIVATE KEY-----` (real newlines, or `\n` on one line) |
| `BEAGLE_APNS_KEY_ID` | the Key ID |
| `BEAGLE_APNS_TEAM_ID` | the Team ID |

Then give `beagle serve` a command that prints them, as a JSON object or
as `KEY=VALUE` lines (dotenv: `export`, comments, single/double quotes and
multi-line quoted values are fine). With Doppler, for example:

```sh
beagle serve secrets sync --from "doppler secrets download --project <project> --config <config> --no-file --format json"
```

`secrets sync` runs the command with `sh -c` (30 s timeout), keeps only the
three names (everything else it prints is dropped), checks they make a
working key, stores each as a generic-password item in the login Keychain
(service `dev.beagletools.serve`, account = the name), saves the command
as `[secrets] command` in `serve.toml`, and restarts the service. Then:

- `beagle serve secrets sync` (no `--from`) reruns the saved command, e.g.
  after rotating the key. `beagle serve` also reruns it at startup, in
  the background: the server answers at once with what the Keychain
  already has, and reloads push when the sync finishes (bounded by the
  timeout). If it fails (offline, logged out) the server logs it and keeps
  what the Keychain has.
- `beagle serve secrets status` lists which names the Keychain holds
  (never values), the saved command, and where push config comes from.
- `beagle serve secrets clear` removes the three items.
- `beagle serve status` and `beagle doctor` name the source. They read the
  Keychain without ever showing a dialog: when macOS wants approval (often
  after an update), they say "Keychain access needs approval — run
  `beagle serve secrets sync`" instead of waiting for a click.

**Where the key comes from**, first match wins:

1. `BEAGLE_APNS_*` in the server's environment (e.g. `doppler run --
   beagle serve` in the foreground; launchd's service has no such
   variables),
2. the Keychain items,
3. `[push]` in `serve.toml` (step 2's file).

The first source holding any of the names is used whole: a partial or
broken one turns push off with a log line rather than falling through to
the next, so you never push with a key you didn't mean to use.

**Security notes.**

- Values are never printed or logged; only the names, the source and the
  two ids are. The key stays in the server's memory; with the Keychain it
  is never written to a file by beagle.
- Values reach the Keychain through the Security framework, never a
  command line (`security add-generic-password -w` would show them in
  `ps`).
- The command is run, not stored output: let the secret manager inject or
  print secrets for the pipe, and don't paste `secrets get` output into
  shells, logs or issues.
- After `brew upgrade beagle`, macOS may ask whether the new binary may
  read the items: choose **Always Allow** (or run `beagle serve secrets
  sync` again, which re-creates them).
- Off macOS there is no Keychain: use the environment or `[push]`.

## Build order

Each step is one PR that leaves everything working:

1. **Events + devices** (serve and scout): the spool writer in scout, the
   follower, `devices.json`, the five device routes (with `test` answering
   `503 push_not_configured`), the catalog from scout's crate. Testable
   without Apple: a fake sender records what it would send.
2. **The APNs sender:** `[push]` config, `push setup`, JWT signing (tested
   against a known key and vector), the payload builder, response handling
   (table-driven tests against a fake HTTP/2 server or a mocked transport),
   `push test`.
3. **The app:** entitlements, permission, registration, the settings
   section in BeagleUI, tap routing, Unpair → `DELETE`. UI tests against the
   in-app demo server, which gains the device routes.
4. **The key** (one-time Apple setup, above), then an end-to-end check on the phone.

## Out of scope for v1

- Badges, quiet hours, and per-project mute.
- Time-sensitive or critical alerts.
- Rich notifications (images) and actions on the banner (for example
  "Mark unblocked"). Actions would be the first phone-side *writes* to a
  tool's store, so they need their own design.
- Delivery when the Mac is asleep or `beagle serve` is down. Events wait up
  to 10 minutes, then are dropped (section 1).

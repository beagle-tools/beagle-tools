# beagle serve

Your Mac fetches; your phone catches. `beagle serve` hands every beagle
tool's data to the [Beagle iPhone app](../../ios/README.md): read-only JSON
over real HTTPS, reachable only over your Tailscale network, behind a bearer
token. It can also send push notifications through Apple when an agent
needs you. The Mac side is built, and the app's side is still to come
(see [Push notifications](#push-notifications)).

```text
iPhone (Beagle app) ──HTTPS + token, over Tailscale──▶ beagle serve (your Mac) ──▶ each tool's store
                    ◀──── push, through Apple (APNs) ─────┘
```

It reads each store through that tool's own crate, the way `beagle <tool>`
does, so the phone sees exactly what your terminal sees. Nothing on the
phone writes to a tool's store.

It ships inside the `beagle` binary ([`../README.md`](../README.md)); this
crate is `beagle-serve`. The API contract is
[`docs/serve-api.md`](../../docs/serve-api.md); push is designed in
[`docs/serve-push.md`](../../docs/serve-push.md).

## What you need

- macOS for the background service (`install`, `uninstall`, `status` are
  macOS-only; the foreground server runs anywhere).
- [Tailscale](https://tailscale.com) on the Mac and the phone, signed in to
  the same tailnet, with **MagicDNS** on and **HTTPS Certificates** enabled
  (admin console → DNS → HTTPS Certificates).
- For push only: an Apple developer account with an APNs key.

## Set it up

**1. Make the token and learn your Mac's name:**

```sh
beagle serve pair
```

The first run makes the token, asks Tailscale for this Mac's MagicDNS name
(something like `my-mac.example-tailnet.ts.net`) and saves both to
`~/.config/beagle/serve.toml`. It shows a QR code, but don't scan it yet:
the app checks that the server answers before it saves anything, and the
server isn't running. Because there's no certificate yet, it ends by
printing the exact `tailscale cert` command for step 2.

**2. Make the certificate:** create the folder, then run the command it
printed. Together they look like this:

```sh
mkdir -p ~/.config/beagle/serve
tailscale cert \
  --cert-file ~/.config/beagle/serve/my-mac.example-tailnet.ts.net.crt \
  --key-file  ~/.config/beagle/serve/my-mac.example-tailnet.ts.net.key \
  my-mac.example-tailnet.ts.net
```

**3. Run it in the background** (a launchd agent that starts at login and
restarts if it exits):

```sh
beagle serve install
beagle serve status       # or `beagle doctor`
```

**4. Pair the phone.** Run `beagle serve pair` again (same token, same QR
code) and scan it with the Beagle app
([`ios/README.md`](../../ios/README.md), "Pair it with your Mac"). It also
prints the server URL and token as text, if you'd rather type them.

That's it. Test it from the Mac (the token is in `serve.toml`):

```sh
curl https://my-mac.example-tailnet.ts.net:8787/v1/health        # no token needed
curl -H "Authorization: Bearer <token>" \
  https://my-mac.example-tailnet.ts.net:8787/v1/scout/projects
```

## Commands

```sh
beagle serve [--addr <ip>] [--port <n>] [--cert <file>] [--key <file>] [--any-addr] [--no-tls]
                                     # run in the foreground (logs to stderr)
beagle serve pair [--rotate] [--host <name>] [--port <n>]
                                     # the QR code; --rotate makes a new token (logs out every phone)
beagle serve install                 # background launchd agent; run again to restart it
beagle serve uninstall               # stop and remove it
beagle serve status                  # running? url, token set?, phones registered, push state
beagle serve push setup --key <AuthKey_….p8> --key-id <id> --team-id <id>
beagle serve push test [--device <id>]
beagle serve secrets sync [--from "<command>"]   # push key from a secret manager into the Keychain
beagle serve secrets status | clear
```

`beagle serve --help` is the authoritative list.

## Push notifications

When a scout task is blocked (and the other events you turn on), the phone
gets a real notification, even with the app closed. The Mac calls Apple's
push service directly with your own APNs key: no relay, no third party.

1. In your Apple developer account: Certificates, IDs & Profiles → Keys →
   **+**, tick **Apple Push Notifications service (APNs)**, download
   `AuthKey_<KEYID>.p8` (Apple lets you download it once). Note the Key ID
   and your Team ID (10 characters each).
2. On the Mac:

   ```sh
   beagle serve push setup --key ~/Downloads/AuthKey_KEY1234567.p8 \
     --key-id KEY1234567 --team-id ABC123DEFG
   ```

   It checks the key, copies it to `~/.config/beagle/serve/` with mode
   `0600`, writes `[push]` to `serve.toml` and restarts the service. Delete
   the download afterwards.

   Or keep the key in a secret manager: store `BEAGLE_APNS_KEY`,
   `BEAGLE_APNS_KEY_ID` and `BEAGLE_APNS_TEAM_ID` there and run
   `beagle serve secrets sync --from "<command that prints them>"`; they go
   to the macOS Keychain, not a file
   ([`docs/serve-push.md`](../../docs/serve-push.md), "Keeping the key in a
   secret manager"). The key is taken from the environment first, then the
   Keychain, then `[push]`.
3. `beagle serve push test` sends "Beagle · test push" to every registered
   phone and prints Apple's answer.

Phones register themselves through the device API. **Status:** the Mac side
(event spool, device API, APNs sender, `push setup`/`test`) is in; the
iPhone app's registration and notification settings are not built yet
([`docs/serve-push.md`](../../docs/serve-push.md), "Build order"), so
`beagle serve status` shows 0 phones until they land. Without `[push]`,
the server logs the notifications it would have sent.

## Files

| Path | What it holds |
|---|---|
| `~/.config/beagle/serve.toml` | `token`, `host`, `port`, optional `addr`/`cert`/`key`, `[push]`. Mode `0600` (it holds the token); unknown keys are an error. `$XDG_CONFIG_HOME` moves it. |
| `~/.config/beagle/serve/<host>.crt`, `.key` | the `tailscale cert` output |
| `~/.config/beagle/serve/AuthKey_<id>.p8` | the APNs key, copied by `push setup` |
| `~/Library/LaunchAgents/com.beagle.serve.plist` | the background service |
| `~/.local/state/beagle/serve/serve.{out,err}.log` | its logs; the first lines name the store each tool resolved to |
| `~/.local/state/beagle/serve/devices.json` | phones registered for push |
| `~/.local/state/beagle/events/scout.jsonl` | scout's event spool, which the server follows for push |

```toml
# ~/.config/beagle/serve.toml
token = "…64 hex characters…"           # written by `beagle serve pair`
host = "my-mac.example-tailnet.ts.net"  # MagicDNS name, for the cert and the pairing URL
port = 8787
```

The service copies your shell's store variables (`BEAGLE_SCOUT_ROOT`,
`BEAGLE_RECAP_ROOT`, …, listed in
[`serve-api.md`](../../docs/serve-api.md#running-it)) when you run
`install`. Change a store location later? Run `beagle serve install` again.

## Troubleshooting

| Symptom | Fix |
|---|---|
| `no token yet` | `beagle serve pair` |
| `tailscale: … is Tailscale running and signed in?` | Start Tailscale; or set `host` (and `addr`) in `serve.toml`. MagicDNS must be on for the name. |
| It exits naming a `tailscale cert` command | The cert or key is missing: run that command. |
| The phone can't connect | Tailscale on, on the phone too; `beagle serve status` shows "running"; `curl …/v1/health` from the Mac works. Port `8787` unless you changed it. |
| TLS errors after ~90 days | `tailscale cert` certificates expire: re-run step 2, then `beagle serve install` to restart. |
| `beagle doctor` shows `✗ beagle serve: loaded but …` | Read `~/.local/state/beagle/serve/serve.err.log`; launchd retries every 30 s. |
| The phone shows an empty or wrong store | The service reads the store variables from when you ran `install`: re-run it from a shell where `beagle scout root` is right. |
| `… listens on every interface` / `--no-tls … only allowed on loopback` | On purpose: pass `--any-addr` if you mean it; plain HTTP is for `127.0.0.1` testing only. |
| A phone was lost, or the token leaked | `beagle serve pair --rotate`: new token, every phone logged out, service restarted. |
| `push: not set up` in `doctor` | Only a warning; see "Push notifications". |

## For agents

- Local testing without Tailscale:
  `beagle serve --no-tls --addr 127.0.0.1 --port 18787` with a throwaway
  `XDG_CONFIG_HOME` holding a fake token
  ([`ios/AGENTS.md`](../../ios/AGENTS.md), "Refreshing fixtures").
- Never print, log or commit the token, a `.p8`, a real tailnet name, a
  Team ID or a device id. Examples use `my-mac.example-tailnet.ts.net`,
  `ABC123DEFG` and `KEY1234567`.
- Every request but `GET /v1/health` needs `Authorization: Bearer <token>`.
  Endpoints and shapes: [`docs/serve-api.md`](../../docs/serve-api.md).

## Develop

```sh
make check-beagle                          # from the repo root
cargo run -p beagle-cli -- serve --help
```

Module layout is in `src/lib.rs`; the crate follows the `beagle` tool
contract ([`../AGENTS.md`](../AGENTS.md)).

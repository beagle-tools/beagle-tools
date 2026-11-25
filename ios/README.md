# Beagle for iPhone

Your beagle tools, off the leash: one read-only iPhone app for every beagle
tool, live from your Mac. It opens on a home screen of tools:
[Scout](../scout/README.md) (your projects, their tasks and the project log)
today; recap, standup, rca, tandem and agent show as "coming soon" until
they get screens. The app never reads files itself; it asks
[`beagle serve`](../beagle/serve/README.md) on your Mac over HTTPS
([`docs/serve-api.md`](../docs/serve-api.md)).

```text
iPhone (this app) ──HTTPS + token, over Tailscale──▶ beagle serve (your Mac) ──▶ each tool's store
```

It sits at the repo root, not inside a tool, because it is the app for every
tool ([`docs/LAYOUT.md`](../docs/LAYOUT.md), rule 9).

## What you need

- A Mac with Xcode 16 or newer, and an Apple developer account (a free
  personal team works for running on your own phone; push notifications
  need a paid one).
- An iPhone on iOS 17 or newer.
- [Tailscale](https://tailscale.com) on the Mac and the phone, signed in to
  the same tailnet, with MagicDNS and HTTPS Certificates on.
- `beagle` on the Mac (`brew install beagle-tools/tap/beagle`,
  [`beagle/README.md`](../beagle/README.md)).

## Install on your iPhone

There is no App Store or TestFlight build: you build it from Xcode onto your
own phone with your Apple developer account.

1. Open `ios/Beagle.xcodeproj` in Xcode. It links BeagleKit from
   `../utils/swift` as a local package, so open it from a full checkout.
2. Select the **Beagle** target → **Signing & Capabilities** → pick your
   **Team**. Signing is automatic; the repo never stores a team ID, so Xcode
   keeps your choice locally. If the bundle ID `dev.beagletools.beagle` is taken
   for your team, change it there too (and don't commit it).
3. Plug in the phone (or pair it over Wi-Fi), pick it as the run destination,
   and press Run. The first time, turn on Developer Mode if iOS asks, and
   trust the developer profile on the phone (Settings → General → VPN &
   Device Management).

The app was called Scout (bundle ID `dev.beagletools.scout`) before it became
Beagle. The new bundle ID installs as a separate app: delete the old Scout app
from the phone and pair Beagle once.

**Just want to look around?** Run it in the Simulator with the launch
argument `-demo-server` (Product → Scheme → Edit Scheme → Run → Arguments;
Debug builds only). It pairs with a fake `beagle serve` built into the app,
with neutral example data, so no Mac setup is needed.

## Pair it with your Mac

1. Set up `beagle serve` on the Mac: a Tailscale certificate, a token, and
   the background service. The steps are in
   [`beagle/serve/README.md`](../beagle/serve/README.md#set-it-up); the short
   version is `beagle serve pair` (makes the token, prints the
   `tailscale cert` command to run), that `tailscale cert` command, then
   `beagle serve install`. Tailscale must be on, on the phone too.
2. On the Mac: `beagle serve pair`. It shows a QR code.
3. In the app: the gear on the home screen → **Pair with your Mac** → **Scan
   the QR code**. Or scan it with the iPhone Camera app, which offers to open
   it in Beagle; or paste the `beagle://pair?…` link, or type the server
   (`https://my-mac.example-tailnet.ts.net:8787`) and the token, then tap
   **Pair**.

The app checks the server answers and accepts the token before saving
anything, so `beagle serve` must be running when you pair. The token is kept
in the iOS Keychain (this device only); Settings shows the paired Mac and
whether it answers, with **Pair again** and **Unpair**.
`beagle serve pair --rotate` on the Mac logs every phone out; pair again
afterwards.

## Push notifications

Push is half built. The Mac side is in: once you give `beagle serve` an
APNs key (`beagle serve push setup …`,
[`beagle/serve/README.md`](../beagle/serve/README.md#push-notifications)),
it follows scout's events and sends to every registered phone. The app
side (the Push capability, asking permission, registering with the Mac,
per-tool notification settings, tap to open the task) is not built yet,
so today no phone registers and nothing arrives. The plan is
[`docs/serve-push.md`](../docs/serve-push.md), section 4 "The app".

## Troubleshooting

The app's own error messages say what to check; the common ones:

| The app says | Fix |
|---|---|
| "Can't find your Mac. Is Tailscale connected on this phone?" | Turn Tailscale on, on the phone; same tailnet as the Mac. |
| "Your Mac refused the connection. Is `beagle serve` running?" | `beagle serve status` on the Mac; `beagle serve install` to (re)start it. |
| "Your Mac didn't answer in time…" | The Mac is asleep, or Tailscale or `beagle serve` is off there. |
| "The Mac's HTTPS certificate isn't trusted or has expired…" | Certificates from `tailscale cert` last 90 days: make a new one and re-run `beagle serve install` ([`beagle/serve/README.md`](../beagle/serve/README.md#troubleshooting)). |
| "Your Mac rejected this phone's token…" | The token was rotated: `beagle serve pair` on the Mac, then pair again. |
| "The server address must start with https://." | Type the `https://` URL `beagle serve pair` prints, not an IP. |
| Xcode: signing or provisioning errors | Pick your Team under Signing & Capabilities; change the bundle ID locally if it's taken. Never commit either. |

## App icon

`Beagle/Assets.xcassets/AppIcon.appiconset/AppIcon.png` is the Beagle desktop
app's icon ([`rca/desktop/app-icon.png`](../rca/desktop/app-icon.png), part of
this repo under its [LICENSE](../LICENSE)), made full-bleed for iOS: the cream
rounded square is filled out to a 1024×1024 opaque square with no baked-in
corners (iOS rounds them), and the dog keeps the size and position it has on
the desktop icon. There is one universal image, and iOS derives the dark and
tinted versions from it. A hand-made dark variant would need the dog cut out of
its cream background, and its chest has no outline to cut along. When the
desktop icon changes, redo it the same way: crop the dog with a cream margin,
scale it by 1024 / 824 (the square's width), and paste it at the same spot on
a `#F5EBD4` square.

## Develop

```sh
make check-ios                      # what CI runs: lint, tests, coverage, docs, analyzer, release build
IOS_SIMULATOR="iPhone 17" make check-ios   # pick the simulator
xcrun swift-format format -i -r --configuration standards/swift/swift-format.json ios utils/swift  # fix formatting
```

The project uses Xcode's synchronized folders: any `.swift` file you add under
`Beagle/`, `BeagleTests/` or `BeagleUITests/` is part of that target, with no
project-file change. A tool's screens go in `Beagle/Tools/<Tool>/` and are
listed in `Beagle/Tools/Tools.swift`. See [`AGENTS.md`](AGENTS.md) for the
code layout.

## For agents

Read [`AGENTS.md`](AGENTS.md) before changing the app. Run UI work against
the in-app demo server (`-demo-server`), not a real Mac. Never commit a
`DEVELOPMENT_TEAM`, a provisioning profile, a bundle-ID override, a real
tailnet name or a token: `make standards` fails the signing ones, and the
rest leak personal data. Fixtures come from a real `beagle serve` over a
neutral throwaway store (AGENTS.md, "Refreshing fixtures").

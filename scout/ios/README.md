# Scout for iPhone

A read-only iPhone app for [`beagle scout`](../tui/README.md): your scout
projects, their tasks and the project log, live from your Mac. The app never
reads files itself; it asks `beagle serve` on your Mac over HTTPS
([`docs/serve-api.md`](../../docs/serve-api.md)).

```text
iPhone (this app) ──HTTPS + token, over Tailscale──▶ beagle serve (your Mac) ──▶ scout store
```

## Install on your iPhone

There is no App Store or TestFlight build: you build it from Xcode onto your
own phone with your Apple developer account.

1. Open `scout/ios/Scout.xcodeproj` in Xcode (16 or newer).
2. Select the **Scout** target → **Signing & Capabilities** → pick your
   **Team**. Signing is automatic; the repo never stores a team ID, so Xcode
   keeps your choice locally. If the bundle ID `dev.beagletools.scout` is taken
   for your team, change it there too (and don't commit it).
3. Plug in the phone (or pair it over Wi-Fi), pick it as the run destination,
   and press Run. The first time, trust the developer profile on the phone
   (Settings → General → VPN & Device Management).

## Pair it with your Mac

1. Set up `beagle serve` on the Mac (Tailscale, `tailscale cert`, `beagle serve
   install`; see [`docs/serve-api.md`](../../docs/serve-api.md)). Tailscale must
   be on, on the phone too.
2. On the Mac: `beagle serve pair`. It shows a QR code.
3. In the app: Settings → **Pair with your Mac** → **Scan the QR code**. Or scan
   it with the iPhone Camera app, which offers to open it in Scout; or paste the
   `beagle://pair?…` link, or type the server and token.

The app checks the server answers and accepts the token before saving
anything. The token is kept in the iOS Keychain. `beagle serve pair --rotate`
on the Mac logs every phone out; pair again afterwards.

## Develop

```sh
make check-ios                      # what CI runs: swift-format lint, unit + UI tests, release build
IOS_SIMULATOR="iPhone 17" make check-ios   # pick the simulator
xcrun swift-format format -i -r scout/ios  # fix formatting (config: the root .swift-format)
```

The project uses Xcode's synchronized folders: any `.swift` file you add under
`Scout/`, `ScoutTests/` or `ScoutUITests/` is part of that target, with no
project-file change. See [`AGENTS.md`](AGENTS.md) for the code layout.

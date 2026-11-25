# Beagle for iPhone

One read-only iPhone app for every beagle tool, live from your Mac. It opens
on a home screen of tools: [Scout](../scout/README.md) (your projects, their
tasks and the project log) today; recap, standup, rca, tandem and agent show
as "coming soon" until they get screens. The app never reads files itself; it
asks `beagle serve` on your Mac over HTTPS
([`docs/serve-api.md`](../docs/serve-api.md)).

```text
iPhone (this app) ──HTTPS + token, over Tailscale──▶ beagle serve (your Mac) ──▶ each tool's store
```

It sits at the repo root, not inside a tool, because it is the app for every
tool ([`docs/LAYOUT.md`](../docs/LAYOUT.md), rule 9).

## Install on your iPhone

There is no App Store or TestFlight build: you build it from Xcode onto your
own phone with your Apple developer account.

1. Open `ios/Beagle.xcodeproj` in Xcode (16 or newer).
2. Select the **Beagle** target → **Signing & Capabilities** → pick your
   **Team**. Signing is automatic; the repo never stores a team ID, so Xcode
   keeps your choice locally. If the bundle ID `dev.beagletools.beagle` is taken
   for your team, change it there too (and don't commit it).
3. Plug in the phone (or pair it over Wi-Fi), pick it as the run destination,
   and press Run. The first time, trust the developer profile on the phone
   (Settings → General → VPN & Device Management).

The app was called Scout (bundle ID `dev.beagletools.scout`) before it became
Beagle. The new bundle ID installs as a separate app: delete the old Scout app
from the phone and pair Beagle once.

## Pair it with your Mac

1. Set up `beagle serve` on the Mac (Tailscale, `tailscale cert`, `beagle serve
   install`; see [`docs/serve-api.md`](../docs/serve-api.md)). Tailscale must
   be on, on the phone too.
2. On the Mac: `beagle serve pair`. It shows a QR code.
3. In the app: the gear on the home screen → **Pair with your Mac** → **Scan
   the QR code**. Or scan it with the iPhone Camera app, which offers to open
   it in Beagle; or paste the `beagle://pair?…` link, or type the server and
   token.

The app checks the server answers and accepts the token before saving
anything. The token is kept in the iOS Keychain. `beagle serve pair --rotate`
on the Mac logs every phone out; pair again afterwards.

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

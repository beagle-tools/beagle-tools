# AGENTS.md — the Beagle iPhone app (`ios/`)

Read the root [`AGENTS.md`](../AGENTS.md) and
[`docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md) first. The
API this app reads is [`docs/serve-api.md`](../docs/serve-api.md); it is
the contract, and the app never reads a tool's store directly.

## What it is

One SwiftUI app (iOS 17+, Swift 6, main-actor by default) for every beagle
tool, read-only. It opens on a home screen of tools; each tool with screens
shows its data the way its TUI does (Scout today), the rest are listed as
"coming soon". No third-party dependencies. It lives at the repo root, not in
a tool folder: it is the app for every tool (`docs/LAYOUT.md`, rule 9).

## Layout

```text
Beagle.xcodeproj/   plain Xcode project; synchronized folders, so files need no project edits;
                    links BeagleKit (../utils/swift) as a local package
  xcshareddata/xcschemes/Beagle.xcscheme   the shared scheme CI tests (unit + UI)
Beagle-Info.plist   merged into the generated Info.plist: only the beagle:// URL scheme
Beagle/             the app target
  BeagleApp.swift   @main entry; makes the Pairing (Keychain service dev.beagletools.beagle);
                    `-ui-testing` swaps in a throwaway Keychain + defaults
  RootView.swift    the one NavigationStack (home → a tool's screens, or Settings);
                    opens beagle://pair links in PairView
  SettingsView.swift  BeagleUI's ServerSection + the app version
  Home/             Tool (one tool on the home screen) and HomeView (the list + Settings)
  Tools/Tools.swift every tool the home screen lists, in order
  Tools/Scout/      scout: ScoutTool (its home-screen entry and ScoutRoot, which declares
                    its routes), API/ (models + endpoints, `extension BeagleAPI`), the
                    screens ProjectListView → ProjectView (Tasks · Project · Log) →
                    TaskView (Task · Outcome), StatusViews (the TUI's glyphs and colours)
  Demo/DemoServer.swift   Debug only: an in-app fake beagle serve (`-demo-server`)
BeagleTests/        unit tests (Swift Testing), hosted by the app; BeagleTestSupport's
                    StubServer; KeychainTests (the real Keychain needs a host app)
  Fixtures/*.json   real `beagle serve` answers for a neutral example store
BeagleUITests/      XCUITest: launch/pairing smoke tests, home + scout screens against
                    the demo server (screenshots kept in the .xcresult), and an
                    opt-in end-to-end test against a real local beagle serve
```

## Adding a tool's screens

1. A folder `Beagle/Tools/<Tool>/` with its models, endpoints
   (`extension BeagleAPI`) and screens.
2. An entry point like `ScoutTool`: a `Tool` whose `screen` returns the
   tool's first view. That view declares the tool's own
   `.navigationDestination`s, so `RootView` stays tool-agnostic.
3. Replace the tool's `.comingSoon(…)` line in `Tools/Tools.swift`.

Anything every tool's screens need (the client, pairing, Keychain, pair
screen, server settings, load state, spinner, Markdown) lives in BeagleKit
(`utils/swift`, see `utils/README.md`), not here.

Settings always replaces the whole navigation stack (it is never under a
tool's screens), so a new pairing only has to drop screens deeper than a
tool's first one.

The app target is main-actor by default: anything a unit test calls
synchronously (models, parsers, pure helpers such as `TaskView.sections`)
must be `nonisolated`, or the hosted test crashes on an isolation check.

## Pairing and secrets

- The token lives only in the Keychain (service `dev.beagletools.beagle`,
  this device only, after first unlock); the server URL in UserDefaults.
- `Pairing.pair` saves nothing until `/v1/health` answers and `/v1/version`
  accepts the token with `api == 1` and `scout` in `tools`.
- A `beagle://` link that opens the app only pre-fills the pair screen; the
  user still taps Pair.
- `PairingLink` accepts `https://` servers, and `http://` only on loopback
  (local testing against `beagle serve --no-tls`).
- Never commit a real tailnet name, IP, token, team ID or bundle-ID
  override; examples use `mac.example-tailnet.ts.net` and `ABC123DEFG`.

## Refreshing fixtures and the end-to-end test

Fixtures are captured from a real server, never hand-written: build a
throwaway scout store with neutral data (`beagle scout init`, `new`,
`task add` …), point a throwaway `XDG_CONFIG_HOME/beagle/serve.toml` at it
with a fake token, run `beagle serve --no-tls --addr 127.0.0.1 --port 18787`,
and `curl` each endpoint into `BeagleTests/Fixtures/`. With that server up,
the UI test `testPairsWithALocalServer` pairs the app with it:

```sh
TEST_RUNNER_SCOUT_E2E_SERVER=http://127.0.0.1:18787 \
TEST_RUNNER_SCOUT_E2E_TOKEN=<the fake token> make check-ios
```

Add `TEST_RUNNER_SCOUT_E2E_PROJECT=<id>`, `TEST_RUNNER_SCOUT_E2E_TASK=<task id>`
and `TEST_RUNNER_SCOUT_E2E_TASK_PROGRESS="1 of 5 done"` to also open that
project and check the task row against `beagle scout show`.

It skips itself without those variables (as in CI).

## Rules

- **Gate:** `make check-ios` (= `scripts/ci/ios.sh ios`, the CI job
  `ios`): `swift-format lint --strict` with
  `standards/swift/swift-format.json` (no force unwraps or `try!` outside
  tests, every public declaration documented), unit and UI tests on an iPhone
  simulator, `xcodebuild analyze`, then an unsigned Release build. The project
  builds on `standards/swift/Shared.xcconfig` (Swift 6, complete concurrency,
  warnings are errors) and may not override it. See
  `docs/CODING_STANDARDS.md`, section Swift.
- **400 lines per file** applies to `.swift` too (`scripts/check-file-length.sh`).
- **Signing stays automatic with no team ID.** Never commit
  `DEVELOPMENT_TEAM`, a provisioning profile or manual signing;
  `make standards` fails it (`scripts/ci/ios-standards.mjs`).
- **No personal data:** no real hostnames, tailnet names or tokens in code,
  tests or fixtures. Use `mac.example.ts.net` and obviously fake tokens.
- Test fixtures copy the JSON shapes in `docs/serve-api.md` with neutral
  example data.

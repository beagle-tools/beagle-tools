# AGENTS.md — scout iPhone app (`scout/ios/`)

Read the root [`AGENTS.md`](../../AGENTS.md) and
[`docs/CODING_STANDARDS.md`](../../docs/CODING_STANDARDS.md) first. The
API this app reads is [`docs/serve-api.md`](../../docs/serve-api.md); it is
the contract, and the app never reads the scout store directly.

## What it is

A SwiftUI app (iOS 17+, Swift 6, main-actor by default) that shows the scout
store read-only, the way the `beagle scout` TUI does. No third-party
dependencies.

## Layout

```text
Scout.xcodeproj/    plain Xcode project; synchronized folders, so files need no project edits;
                    links BeagleKit (../../utils/swift) as a local package
  xcshareddata/xcschemes/Scout.xcscheme   the shared scheme CI tests (unit + UI)
Scout-Info.plist    merged into the generated Info.plist: only the beagle:// URL scheme
Scout/              the app target
  ScoutApp.swift    @main entry; makes the Pairing (Keychain service dev.beagletools.scout);
                    `-ui-testing` swaps in a throwaway Keychain + defaults
  RootView.swift    the tabs: Scout, Settings; opens beagle://pair links in PairView
  SettingsView.swift  BeagleUI's ServerSection + the app version
  API/              the scout models and endpoints (`extension BeagleAPI`)
  Scout/            the screens: ProjectListView → ProjectView (Tasks · Project · Log)
                    → TaskView (Task · Outcome); StatusViews (the TUI's glyphs and
                    colours); Routes
  Demo/DemoServer.swift   Debug only: an in-app fake beagle serve (`-demo-server`)
ScoutTests/         unit tests (Swift Testing), hosted by the app; BeagleTestSupport's
                    StubServer; KeychainTests (the real Keychain needs a host app)
  Fixtures/*.json   real `beagle serve` answers for a neutral example store
ScoutUITests/       XCUITest: launch/pairing smoke tests, the scout screens against
                    the demo server (screenshots kept in the .xcresult), and an
                    opt-in end-to-end test against a real local beagle serve
```

Anything another beagle phone app would need (the client, pairing, Keychain,
pair screen, server settings, load state, spinner, Markdown) lives in
BeagleKit (`utils/swift`, see `utils/README.md`), not here.

The app target is main-actor by default: anything a unit test calls
synchronously (models, parsers, pure helpers such as `TaskView.sections`)
must be `nonisolated`, or the hosted test crashes on an isolation check.

## Pairing and secrets

- The token lives only in the Keychain (service `dev.beagletools.scout`,
  this device only, after first unlock); the server URL in UserDefaults.
- `Pairing.pair` saves nothing until `/v1/health` answers and `/v1/version`
  accepts the token with `api == 1` and `scout` in `tools`.
- A `beagle://` link that opens the app only pre-fills the pair screen; the
  user still taps Pair.
- `PairingLink` accepts `https://` servers, and `http://` only on loopback
  (local testing against `beagle serve --no-tls`).

## Refreshing fixtures and the end-to-end test

Fixtures are captured from a real server, never hand-written: build a
throwaway scout store with neutral data (`beagle scout init`, `new`,
`task add` …), point a throwaway `XDG_CONFIG_HOME/beagle/serve.toml` at it
with a fake token, run `beagle serve --no-tls --addr 127.0.0.1 --port 18787`,
and `curl` each endpoint into `ScoutTests/Fixtures/`. With that server up,
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

- **Gate:** `make check-ios` (= `scripts/ci/ios.sh scout/ios`, the CI job
  `scout-ios`): `swift-format lint --strict` with the root `.swift-format`,
  then `xcodebuild test` on an iPhone simulator, then an unsigned Release
  build. Warnings are errors.
- **400 lines per file** applies to `.swift` too (`scripts/check-file-length.sh`).
- **Signing stays automatic with no team ID.** Never commit
  `DEVELOPMENT_TEAM`, a provisioning profile or manual signing;
  `make standards` fails it (`scripts/ci/ios-standards.mjs`).
- **No personal data:** no real hostnames, tailnet names or tokens in code,
  tests or fixtures. Use `mac.example.ts.net` and obviously fake tokens.
- Test fixtures copy the JSON shapes in `docs/serve-api.md` with neutral
  example data.

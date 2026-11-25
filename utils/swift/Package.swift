// swift-tools-version: 6.2
// BeagleKit: Swift code shared by the beagle iPhone apps, the way the Rust
// tools share utils/ (beagle-utils, beagle-tui). See utils/README.md.

import PackageDescription

// The shared Swift settings for every target (docs/CODING_STANDARDS.md, Swift):
// Swift 6 language mode, so complete concurrency checking. `make standards`
// checks this line and that every target uses it.
//
// Warnings are errors too, but not set here: an app building this package as a
// dependency gets -suppress-warnings from Xcode, which conflicts with
// treatAllWarnings. The package's own gate (scripts/ci/ios.sh) passes
// SWIFT_TREAT_WARNINGS_AS_ERRORS=YES instead.
let strict: [SwiftSetting] = [.swiftLanguageMode(.v6)]

let package = Package(
  name: "BeagleKit",
  platforms: [.iOS(.v17)],
  products: [
    .library(name: "BeagleAPI", targets: ["BeagleAPI"]),
    .library(name: "BeaglePairing", targets: ["BeaglePairing"]),
    .library(name: "BeaglePush", targets: ["BeaglePush"]),
    .library(name: "BeagleUI", targets: ["BeagleUI"]),
    .library(name: "BeagleTestSupport", targets: ["BeagleTestSupport"]),
  ],
  targets: [
    // The `beagle serve` client: transport, auth, errors, server models.
    .target(name: "BeagleAPI", swiftSettings: strict),
    // Pairing with a Mac: beagle://pair links, the Keychain, verification.
    .target(name: "BeaglePairing", dependencies: ["BeagleAPI"], swiftSettings: strict),
    // Push notifications: permission, the APNs token, registering with the
    // Mac and the per-tool settings there (docs/serve-push.md).
    .target(
      name: "BeaglePush", dependencies: ["BeagleAPI", "BeaglePairing"], swiftSettings: strict),
    // SwiftUI pieces: pair screen, QR scanner, server and notification
    // settings, load state, the TUIs' spinner, Markdown.
    .target(
      name: "BeagleUI", dependencies: ["BeagleAPI", "BeaglePairing", "BeaglePush"],
      swiftSettings: strict),
    // For tests only: a fake `beagle serve` behind URLProtocol.
    .target(name: "BeagleTestSupport", dependencies: ["BeagleAPI"], swiftSettings: strict),

    .testTarget(
      name: "BeagleAPITests", dependencies: ["BeagleAPI", "BeagleTestSupport"],
      resources: [.copy("Fixtures")], swiftSettings: strict),
    .testTarget(
      name: "BeaglePairingTests", dependencies: ["BeaglePairing", "BeagleTestSupport"],
      resources: [.copy("Fixtures")], swiftSettings: strict),
    .testTarget(
      name: "BeaglePushTests", dependencies: ["BeaglePush", "BeagleTestSupport"],
      swiftSettings: strict),
    .testTarget(
      name: "BeagleUITests", dependencies: ["BeagleUI", "BeagleTestSupport"],
      swiftSettings: strict),
  ]
)

// swift-tools-version: 6.0
// BeagleKit: Swift code shared by the beagle iPhone apps, the way the Rust
// tools share utils/ (beagle-utils, beagle-tui). See utils/README.md.

import PackageDescription

let package = Package(
  name: "BeagleKit",
  platforms: [.iOS(.v17)],
  products: [
    .library(name: "BeagleAPI", targets: ["BeagleAPI"]),
    .library(name: "BeaglePairing", targets: ["BeaglePairing"]),
    .library(name: "BeagleUI", targets: ["BeagleUI"]),
    .library(name: "BeagleTestSupport", targets: ["BeagleTestSupport"]),
  ],
  targets: [
    // The `beagle serve` client: transport, auth, errors, server models.
    .target(name: "BeagleAPI"),
    // Pairing with a Mac: beagle://pair links, the Keychain, verification.
    .target(name: "BeaglePairing", dependencies: ["BeagleAPI"]),
    // SwiftUI pieces: pair screen, QR scanner, server settings, load state,
    // the TUIs' spinner, Markdown.
    .target(name: "BeagleUI", dependencies: ["BeagleAPI", "BeaglePairing"]),
    // For tests only: a fake `beagle serve` behind URLProtocol.
    .target(name: "BeagleTestSupport", dependencies: ["BeagleAPI"]),

    .testTarget(
      name: "BeagleAPITests", dependencies: ["BeagleAPI", "BeagleTestSupport"],
      resources: [.copy("Fixtures")]),
    .testTarget(
      name: "BeaglePairingTests", dependencies: ["BeaglePairing", "BeagleTestSupport"],
      resources: [.copy("Fixtures")]),
    .testTarget(name: "BeagleUITests", dependencies: ["BeagleUI"]),
  ]
)

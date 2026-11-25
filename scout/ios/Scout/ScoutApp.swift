import SwiftUI

/// The Scout iPhone app: a read-only view of the scout store, served live
/// by `beagle serve` on the Mac (docs/serve-api.md).
@main
struct ScoutApp: App {
  @State private var pairing = ScoutApp.makePairing()

  var body: some Scene {
    WindowGroup {
      RootView()
        .environment(pairing)
    }
  }

  /// UI tests launch with `-ui-testing`: a throwaway Keychain service and
  /// defaults suite, so they never see or change a real pairing.
  private static func makePairing() -> Pairing {
    #if DEBUG
      if CommandLine.arguments.contains("-demo-server") { return Pairing.demo() }
    #endif
    guard CommandLine.arguments.contains("-ui-testing") else { return Pairing() }
    let suite = "dev.beagletools.scout.uitests"
    let defaults = UserDefaults(suiteName: suite) ?? .standard
    defaults.removePersistentDomain(forName: suite)
    let keychain = Keychain(service: suite)
    try? keychain.delete(Pairing.tokenAccount)
    return Pairing(keychain: keychain, defaults: defaults)
  }
}

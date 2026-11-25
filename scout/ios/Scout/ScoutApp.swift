import BeaglePairing
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

  /// The Keychain service the token is filed under (unchanged since the
  /// first version, so an existing pairing survives updates).
  static let keychainService = "dev.beagletools.scout"

  /// The app's pairing.
  ///
  /// UI tests launch with `-ui-testing` (a throwaway Keychain service and defaults suite, so they
  /// never see or change a real pairing) or `-demo-server` (Debug only: paired with the in-app
  /// DemoServer).
  private static func makePairing() -> Pairing {
    #if DEBUG
      if CommandLine.arguments.contains("-demo-server") {
        return Pairing(tool: "scout", preview: DemoServer.link, session: DemoServer.session)
      }
    #endif
    guard CommandLine.arguments.contains("-ui-testing") else {
      return Pairing(tool: "scout", secrets: Keychain(service: keychainService))
    }
    let suite = "dev.beagletools.scout.uitests"
    let defaults = UserDefaults(suiteName: suite) ?? .standard
    defaults.removePersistentDomain(forName: suite)
    let keychain = Keychain(service: suite)
    try? keychain.delete(Pairing.tokenAccount)
    return Pairing(tool: "scout", secrets: keychain, defaults: defaults)
  }
}

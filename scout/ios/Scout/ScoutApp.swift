import SwiftUI

/// The Scout iPhone app: a read-only view of the scout store, served live
/// by `beagle serve` on the Mac (docs/serve-api.md).
@main
struct ScoutApp: App {
  var body: some Scene {
    WindowGroup {
      RootView()
    }
  }
}

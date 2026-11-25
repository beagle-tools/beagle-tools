import SwiftUI

/// The Beagle iPhone app: every beagle tool, read-only.
///
/// Served live by `beagle serve` on the Mac (docs/serve-api.md); it opens on a home screen of
/// tools. The app delegate owns the pairing and push notifications (docs/serve-push.md).
@main
struct BeagleApp: App {
  @UIApplicationDelegateAdaptor(AppDelegate.self) private var app

  var body: some Scene {
    WindowGroup {
      RootView()
        .environment(app.pairing)
        .environment(app.push)
        .environment(app.links)
    }
  }
}

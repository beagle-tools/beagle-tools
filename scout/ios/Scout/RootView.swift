import BeaglePairing
import BeagleUI
import SwiftUI

/// The app's tabs: Scout (projects and tasks) and Settings (pairing).
struct RootView: View {
  @Environment(Pairing.self) private var pairing
  @State private var tab = Tab.scout
  @State private var path = NavigationPath()
  /// A `beagle://pair?…` link that opened the app, waiting for confirmation.
  @State private var openedLink: OpenedLink?

  enum Tab: Hashable { case scout, settings }

  struct OpenedLink: Identifiable {
    let id = UUID()
    let text: String
  }

  var body: some View {
    TabView(selection: $tab) {
      NavigationStack(path: $path) {
        Group {
          if let api = pairing.api {
            ProjectListView(api: api)
          } else {
            ContentUnavailableView {
              Label("Not paired", systemImage: "binoculars")
            } description: {
              Text("Pair with `beagle serve` on your Mac to see your scout projects.")
            } actions: {
              Button("Open Settings") { tab = .settings }
            }
            .navigationTitle("Scout")
          }
        }
        .navigationDestination(for: ProjectRoute.self) { route in
          if let api = pairing.api { ProjectView(api: api, route: route) }
        }
        .navigationDestination(for: TaskRoute.self) { route in
          if let api = pairing.api { TaskView(api: api, route: route) }
        }
      }
      // A new pairing starts from the project list, not a stale screen.
      .onChange(of: pairing.current) { path = NavigationPath() }
      .tabItem { Label("Scout", systemImage: "binoculars") }
      .tag(Tab.scout)

      NavigationStack {
        SettingsView()
      }
      .tabItem { Label("Settings", systemImage: "gear") }
      .tag(Tab.settings)
    }
    // Scanning the QR code with the iPhone Camera app opens this link here.
    // Pairing still waits for a tap on Pair, so a stray link can't repoint the app.
    .onOpenURL { url in
      guard url.scheme?.lowercased() == "beagle" else { return }
      openedLink = OpenedLink(text: url.absoluteString)
    }
    .sheet(item: $openedLink) { opened in
      PairView(initialLink: opened.text)
    }
  }
}

/// Facts about the running build, read from its Info.plist.
enum AppInfo {
  static var version: String {
    let info = Bundle.main.infoDictionary
    let short = info?["CFBundleShortVersionString"] as? String ?? "?"
    let build = info?["CFBundleVersion"] as? String ?? "?"
    return "\(short) (\(build))"
  }
}

#if DEBUG
  #Preview {
    RootView().environment(
      Pairing(tool: "scout", preview: DemoServer.link, session: DemoServer.session))
  }
#endif

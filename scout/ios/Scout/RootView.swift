import SwiftUI

/// The app's tabs: Scout (projects and tasks) and Settings (pairing).
struct RootView: View {
  @Environment(Pairing.self) private var pairing
  @State private var tab = Tab.scout
  /// A `beagle://pair?…` link that opened the app, waiting for confirmation.
  @State private var openedLink: OpenedLink?

  enum Tab: Hashable { case scout, settings }

  struct OpenedLink: Identifiable {
    let id = UUID()
    let text: String
  }

  var body: some View {
    TabView(selection: $tab) {
      NavigationStack {
        Group {
          if pairing.current == nil {
            ContentUnavailableView {
              Label("Not paired", systemImage: "binoculars")
            } description: {
              Text("Pair with `beagle serve` on your Mac to see your scout projects.")
            } actions: {
              Button("Open Settings") { tab = .settings }
            }
          } else {
            ContentUnavailableView(
              "Projects are coming",
              systemImage: "binoculars",
              description: Text("Paired. The project list arrives in the next version."))
          }
        }
        .navigationTitle("Scout")
      }
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

#Preview {
  RootView().environment(Pairing())
}

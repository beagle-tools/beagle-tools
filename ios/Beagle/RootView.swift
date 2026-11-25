import BeaglePairing
import BeagleUI
import SwiftUI

/// The app's one navigation stack: the home screen of tools, each tool's screens, and Settings.
struct RootView: View {
  @Environment(Pairing.self) private var pairing
  @State private var path = NavigationPath()
  /// A `beagle://pair?…` link that opened the app, waiting for confirmation.
  @State private var openedLink: OpenedLink?

  struct OpenedLink: Identifiable {
    let id = UUID()
    let text: String
  }

  var body: some View {
    NavigationStack(path: $path) {
      HomeView(path: $path)
        .navigationDestination(for: HomeRoute.self) { route in
          switch route {
          case .settings:
            SettingsView()
          case .tool(let id):
            toolScreen(id)
          }
        }
    }
    // A new pairing drops screens deeper than a tool's first one, which showed the old server.
    // Settings (always alone on the stack) and a tool's first screen stay; the latter reloads.
    .onChange(of: pairing.current) {
      if path.count > 1 { path = NavigationPath() }
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

  @ViewBuilder private func toolScreen(_ id: String) -> some View {
    if let tool = Tools.named(id), let screen = tool.screen {
      if let api = pairing.api {
        screen(api).id(pairing.current?.server)
      } else {
        ContentUnavailableView {
          Label("Not paired", systemImage: tool.systemImage)
        } description: {
          Text("Pair with `beagle serve` on your Mac to see \(tool.name).")
        } actions: {
          // Settings replaces the stack, so it is never under a tool's screens.
          Button("Open Settings") { path = NavigationPath([HomeRoute.settings]) }
        }
        .navigationTitle(tool.name)
      }
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

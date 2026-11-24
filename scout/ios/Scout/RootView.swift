import SwiftUI

/// The app's tabs: Scout (projects and tasks) and Settings (pairing).
struct RootView: View {
  var body: some View {
    TabView {
      NavigationStack {
        ContentUnavailableView(
          "No projects yet",
          systemImage: "binoculars",
          description: Text("Pair with beagle serve in Settings to see your scout projects.")
        )
        .navigationTitle("Scout")
      }
      .tabItem { Label("Scout", systemImage: "binoculars") }

      NavigationStack {
        Form {
          Section("Server") {
            Text("Not paired")
              .foregroundStyle(.secondary)
          }
          Section("About") {
            LabeledContent("Version", value: AppInfo.version)
          }
        }
        .navigationTitle("Settings")
      }
      .tabItem { Label("Settings", systemImage: "gear") }
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
  RootView()
}

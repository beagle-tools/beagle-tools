import BeaglePairing
import SwiftUI

/// Where the home screen navigates: a tool's screens, or Settings.
enum HomeRoute: Hashable {
  case tool(String)
  case settings
}

/// The home screen: every beagle tool, each opening its own screens, and Settings.
struct HomeView: View {
  @Environment(Pairing.self) private var pairing
  @Binding var path: NavigationPath

  var body: some View {
    List {
      if pairing.current == nil {
        Section {
          Button {
            path = NavigationPath([HomeRoute.settings])
          } label: {
            Label(
              "Pair with `beagle serve` on your Mac to see your tools.",
              systemImage: "exclamationmark.circle")
          }
          .accessibilityIdentifier("home.notPaired")
        }
      }
      Section("Tools") {
        ForEach(Tools.all) { tool in
          if tool.isAvailable {
            NavigationLink(value: HomeRoute.tool(tool.id)) { ToolRow(tool: tool) }
              .accessibilityIdentifier("tool.\(tool.id)")
          } else {
            ToolRow(tool: tool)
              .accessibilityIdentifier("tool.\(tool.id)")
          }
        }
      }
    }
    .navigationTitle("Beagle")
    .toolbar {
      ToolbarItem(placement: .topBarTrailing) {
        NavigationLink(value: HomeRoute.settings) {
          Label("Settings", systemImage: "gear")
        }
        .accessibilityIdentifier("home.settings")
      }
    }
  }
}

/// A tool's row: icon, name and one line on what it is; dimmed with "Coming soon" when the app has
/// no screens for it yet.
struct ToolRow: View {
  let tool: Tool

  var body: some View {
    HStack(spacing: 14) {
      Image(systemName: tool.systemImage)
        .font(.title2)
        .frame(width: 36)
        .foregroundStyle(tool.isAvailable ? Color.accentColor : .secondary)
      VStack(alignment: .leading, spacing: 3) {
        HStack {
          Text(tool.name).font(.headline)
          if !tool.isAvailable {
            Text("Coming soon")
              .font(.caption2.bold())
              .padding(.horizontal, 6)
              .padding(.vertical, 2)
              .background(.quaternary, in: Capsule())
          }
        }
        Text(tool.summary).font(.subheadline).foregroundStyle(.secondary).lineLimit(2)
      }
    }
    .padding(.vertical, 4)
    .opacity(tool.isAvailable ? 1 : 0.6)
    .accessibilityElement(children: .combine)
  }
}

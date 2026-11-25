import SwiftUI

/// Every tool the home screen lists, in order.
///
/// A tool with screens is a folder here (`Scout/`) with an entry point like ``ScoutTool``; the
/// rest show as coming soon.
enum Tools {
  static let all: [Tool] = [
    ScoutTool.tool,
    .comingSoon(
      "recap", name: "Recap",
      summary: "PR recaps: the diff plus a plain-language write-up of what it fixes and why",
      systemImage: "doc.text.magnifyingglass"),
    .comingSoon(
      "standup", name: "Standup",
      summary: "Standup reports: one overview, then a short write-up per ticket",
      systemImage: "person.3"),
    .comingSoon(
      "rca", name: "RCA",
      summary: "Root-cause analysis workspaces: what broke, why, and how to fix it",
      systemImage: "stethoscope"),
    .comingSoon(
      "tandem", name: "Tandem",
      summary: "Review PRs as a queue, with local agent reviews",
      systemImage: "arrow.triangle.pull"),
    .comingSoon(
      "agent", name: "Agent",
      summary: "Scheduled and on-demand coding agents: jobs, runs and logs",
      systemImage: "cpu"),
  ]

  /// The tool with this id, if the app knows it.
  static func named(_ id: String) -> Tool? { all.first { $0.id == id } }

  /// The navigation stack a `beagle://<tool>/…` link opens: home → the tool → its screens.
  ///
  /// The tool routes its own link shapes. Nil for `beagle://pair` and links no tool knows.
  static func path(for url: URL) -> NavigationPath? {
    guard url.scheme?.lowercased() == "beagle", let id = url.host(), let tool = named(id),
      tool.isAvailable, let routes = tool.routes?(url.pathComponents.filter { $0 != "/" })
    else { return nil }
    var path = NavigationPath()
    path.append(HomeRoute.tool(id))
    for route in routes { path.append(route) }
    return path
  }
}

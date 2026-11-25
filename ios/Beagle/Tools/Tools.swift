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
}

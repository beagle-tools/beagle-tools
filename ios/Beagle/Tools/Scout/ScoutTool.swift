import BeagleAPI
import SwiftUI

/// Scout on the home screen: the project list, and the routes its screens push.
enum ScoutTool {
  static let tool = Tool(
    id: "scout", name: "Scout",
    summary: "Watch agent work as projects of small, ordered tasks",
    systemImage: "binoculars",
    screen: { api in AnyView(ScoutRoot(api: api)) },
    routes: links)

  /// The screens a scout link opens: `projects/<id>` → the project, `projects/<id>/tasks/<task>`
  /// → the project, then the task.
  ///
  /// Titles show the ids until the screens load.
  nonisolated static func links(_ segments: [String]) -> [any Hashable]? {
    guard segments.count >= 2, segments[0] == "projects" else {
      return segments.isEmpty ? [] : nil
    }
    let project = segments[1]
    let projectRoute = ProjectRoute(id: project, title: project)
    if segments.count == 2 { return [projectRoute] }
    guard segments.count == 4, segments[2] == "tasks" else { return nil }
    let task = segments[3]
    return [projectRoute, TaskRoute(project: project, task: task, title: task)]
  }
}

/// Scout's first screen.
///
/// It declares where scout's screens navigate, so the home screen needs no scout-specific routes.
struct ScoutRoot: View {
  let api: BeagleAPI

  var body: some View {
    ProjectListView(api: api)
      .navigationDestination(for: ProjectRoute.self) { route in
        ProjectView(api: api, route: route)
      }
      .navigationDestination(for: TaskRoute.self) { route in
        TaskView(api: api, route: route)
      }
  }
}

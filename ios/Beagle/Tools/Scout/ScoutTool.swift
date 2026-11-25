import BeagleAPI
import SwiftUI

/// Scout on the home screen: the project list, and the routes its screens push.
enum ScoutTool {
  static let tool = Tool(
    id: "scout", name: "Scout",
    summary: "Watch agent work as projects of small, ordered tasks",
    systemImage: "binoculars",
    screen: { api in AnyView(ScoutRoot(api: api)) })
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

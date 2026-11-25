import BeagleAPI
import SwiftUI

/// One beagle tool on the home screen, named like its `beagle <id>` subcommand.
///
/// A tool with screens gives its root screen; one without (yet) shows as "coming soon". Each tool
/// with screens lives in its own folder under `Tools/` and registers itself in ``Tools/all``.
struct Tool: Identifiable {
  /// The `beagle` subcommand, e.g. "scout".
  let id: String
  /// The name on the home screen.
  let name: String
  /// One line on what it is, like `beagle --help`.
  let summary: String
  /// The SF Symbol shown beside it.
  let systemImage: String
  /// The tool's first screen, given the paired server; nil while it has none.
  let screen: (@MainActor (BeagleAPI) -> AnyView)?
  /// The screens a `beagle://<id>/…` link opens past the tool's first one, given the link's path
  /// segments (a tapped notification's link, docs/serve-push.md); nil for a link it doesn't know.
  var routes: (([String]) -> [any Hashable]?)?

  /// Whether the tool has screens in this version of the app.
  var isAvailable: Bool { screen != nil }

  /// A tool the app has no screens for yet.
  static func comingSoon(_ id: String, name: String, summary: String, systemImage: String) -> Tool {
    Tool(id: id, name: name, summary: summary, systemImage: systemImage, screen: nil, routes: nil)
  }
}

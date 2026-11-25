import BeagleAPI
import Foundation

/// The scout endpoints of `beagle serve` (docs/serve-api.md, "Scout").
extension BeagleAPI {
  nonisolated func projects() async throws -> ProjectList {
    try await get(["v1", "scout", "projects"])
  }

  nonisolated func project(_ id: String) async throws -> ProjectDetail {
    try await get(["v1", "scout", "projects", id])
  }

  /// `task` is a number (`2`), a full id (`02-add-allowlist`) or a slug.
  nonisolated func task(project: String, task: String) async throws -> TaskDetail {
    try await get(["v1", "scout", "projects", project, "tasks", task])
  }

  /// The last `lines` lines of the project's log (the server caps it at 2000).
  nonisolated func log(project: String, lines: Int = 200) async throws -> LogTail {
    try await get(
      ["v1", "scout", "projects", project, "log"],
      query: [URLQueryItem(name: "lines", value: String(lines))])
  }
}

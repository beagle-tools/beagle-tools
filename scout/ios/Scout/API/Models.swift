import BeagleAPI
import Foundation

// The scout JSON `beagle serve` answers (docs/serve-api.md, "Scout"); the
// server-wide models are in BeagleKit's BeagleAPI. Keys arrive in
// snake_case and are decoded with `.convertFromSnakeCase`. Timestamps stay
// RFC 3339 strings; `Timestamp` turns them into dates for display.

/// A project's status.
///
/// Unknown values (a newer server) decode as `.unknown` instead of failing the whole response.
nonisolated enum ProjectStatus: String, Decodable, Sendable, CaseIterable {
  case planning, active, paused, done, dropped, unknown

  init(from decoder: Decoder) throws {
    let raw = try decoder.singleValueContainer().decode(String.self)
    self = Self(rawValue: raw) ?? .unknown
  }
}

/// A task's status, with the same `.unknown` fallback.
nonisolated enum TaskStatus: String, Decodable, Sendable, CaseIterable {
  case todo, working, paused, blocked, done, skipped, unknown

  init(from decoder: Decoder) throws {
    let raw = try decoder.singleValueContainer().decode(String.self)
    self = Self(rawValue: raw) ?? .unknown
  }
}

/// `GET /v1/scout/projects`.
nonisolated struct ProjectList: Decodable, Equatable, Sendable {
  let projects: [ProjectSummary]
  let warnings: [String]
}

/// One row of the project list (the rows `beagle scout list --json` prints).
nonisolated struct ProjectSummary: Decodable, Equatable, Sendable, Identifiable {
  let id: String
  let title: String
  let status: ProjectStatus
  let goal: String?
  let tasksClosed: Int
  let tasksTotal: Int
  let working: [String]
  let links: [String]
  let created: String?
  let updated: String?
  let lastActivity: String?
}

/// `{ "done": 1, "total": 4 }`: ticked and total checklist items.
nonisolated struct Checklist: Decodable, Equatable, Sendable {
  let done: Int
  let total: Int
}

/// A task as listed in a project.
nonisolated struct TaskSummary: Decodable, Equatable, Sendable, Identifiable {
  let id: String
  let number: Int
  let slug: String
  let title: String
  let status: TaskStatus
  let checklist: Checklist
  let links: [String]
  let created: String?
  let updated: String?
}

/// The latest task's `## Next steps`, which whoever picks the project up reads first.
nonisolated struct Handoff: Decodable, Equatable, Sendable {
  let task: String
  let markdown: String
}

/// `GET /v1/scout/projects/{id}`.
nonisolated struct ProjectDetail: Decodable, Equatable, Sendable, Identifiable {
  let id: String
  let title: String
  let status: ProjectStatus
  let goal: String?
  let links: [String]
  let created: String?
  let updated: String?
  let lastActivity: String?
  /// `project.md` without its frontmatter, raw Markdown.
  let markdown: String
  let tasks: [TaskSummary]
  let handoff: Handoff?
  let warnings: [String]
}

/// `GET /v1/scout/projects/{id}/tasks/{task}`.
nonisolated struct TaskDetail: Decodable, Equatable, Sendable, Identifiable {
  let project: String
  let id: String
  let number: Int
  let slug: String
  let title: String
  let status: TaskStatus
  let checklist: Checklist
  let links: [String]
  let created: String?
  let updated: String?
  /// The whole task body without its frontmatter, raw Markdown.
  let markdown: String
  let nextSteps: String?
}

/// `GET /v1/scout/projects/{id}/log?lines=`: the tail of `log.md`, oldest first.
nonisolated struct LogTail: Decodable, Equatable, Sendable {
  let project: String
  let lines: [String]
  let truncated: Bool
}

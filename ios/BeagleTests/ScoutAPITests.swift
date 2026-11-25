import BeagleAPI
import BeagleTestSupport
import Foundation
import Testing

@testable import Beagle

/// The scout endpoints (Beagle/Tools/Scout/API/ScoutAPI.swift) against JSON captured from a real `beagle serve`.
///
/// Transport, auth and errors are tested in BeagleKit.
struct ScoutAPITests {
  let stub = StubServer()

  @Test func decodesTheProjectList() async throws {
    stub.serve(["/v1/scout/projects": Fixture.data("projects")])
    let list = try await stub.api().projects()

    #expect(list.warnings.isEmpty)
    #expect(list.projects.map(\.id) == ["rate-limit-the-api", "tidy-the-docs"])
    let first = list.projects[0]
    #expect(first.status == .active)
    #expect(first.goal == "Every public endpoint answers 429 past 100 requests a minute per key.")
    #expect(first.tasksClosed == 1 && first.tasksTotal == 3)
    #expect(first.working == ["02-add-allowlist"])
    #expect(first.links == ["ABC-123"])
    #expect(Timestamp.date(first.lastActivity) != nil)
    // A fresh project: no goal yet, never updated.
    #expect(list.projects[1].goal == nil)
    #expect(list.projects[1].updated == nil)
    #expect(list.projects[1].status == .planning)
  }

  @Test func decodesAProjectWithTasksAndHandoff() async throws {
    stub.serve(["/v1/scout/projects/rate-limit-the-api": Fixture.data("project")])
    let project = try await stub.api().project("rate-limit-the-api")

    #expect(project.title == "Rate-limit the public API")
    #expect(project.markdown.hasPrefix("## Goal"))
    #expect(project.tasks.map(\.number) == [1, 2, 3])
    #expect(project.tasks.map(\.status) == [.done, .working, .todo])
    #expect(project.tasks[1].checklist == Checklist(done: 1, total: 5))
    #expect(project.tasks[2].updated == nil)
    #expect(project.handoff?.task == "01-map-request-path")
  }

  @Test func decodesTasks() async throws {
    stub.serve([
      "/v1/scout/projects/rate-limit-the-api/tasks/1": Fixture.data("task-done"),
      "/v1/scout/projects/rate-limit-the-api/tasks/2": Fixture.data("task-working"),
    ])
    let done = try await stub.api().task(project: "rate-limit-the-api", task: "1")
    #expect(done.id == "01-map-request-path")
    #expect(done.nextSteps?.hasPrefix("Next is 02") == true)

    let working = try await stub.api().task(project: "rate-limit-the-api", task: "2")
    #expect(working.status == .working)
    #expect(working.nextSteps == nil)
    #expect(working.links == ["https://github.com/acme/widgets/pull/42"])
    #expect(working.markdown.contains("- [x] Add the migration"))
  }

  @Test func decodesTheLogAndSendsLines() async throws {
    stub.route { request in
      #expect(request.url?.query() == "lines=3")
      return .init(status: 200, body: Fixture.data("log"))
    }
    let log = try await stub.api().log(project: "rate-limit-the-api", lines: 3)
    #expect(log.lines.count == 3)
    #expect(log.truncated)
  }

  @Test func unexpectedJSONIsADecodingError() async {
    stub.route { _ in .init(status: 200, body: Data(#"{"projects": 3}"#.utf8)) }
    let error = await #expect(throws: APIError.self) { try await stub.api().projects() }
    guard case .decoding(let detail) = error else {
      Issue.record("unexpected \(String(describing: error))")
      return
    }
    #expect(detail.contains("projects"))
  }

  @Test func unknownStatusesDecodeAsUnknown() throws {
    let json = Data(#"["active", "someday"]"#.utf8)
    #expect(try JSONDecoder().decode([ProjectStatus].self, from: json) == [.active, .unknown])
  }

}

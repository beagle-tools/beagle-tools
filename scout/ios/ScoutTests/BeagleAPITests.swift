import Foundation
import Testing

@testable import Scout

struct BeagleAPITests {
  let stub = StubServer()

  @Test func decodesTheProjectList() async throws {
    stub.serve(["/v1/scout/projects": "projects"])
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
    stub.serve(["/v1/scout/projects/rate-limit-the-api": "project"])
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
      "/v1/scout/projects/rate-limit-the-api/tasks/1": "task-done",
      "/v1/scout/projects/rate-limit-the-api/tasks/2": "task-working",
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

  @Test func healthNeedsNoTokenAndEverythingElseSendsIt() async throws {
    stub.route { request in
      let auth = request.value(forHTTPHeaderField: "Authorization")
      switch request.url?.path() {
      case "/v1/health":
        #expect(auth == nil)
        return .init(status: 200, body: Fixture.data("health"))
      default:
        #expect(auth == "Bearer \(Fixture.token)")
        return .init(status: 200, body: Fixture.data("version"))
      }
    }
    let api = stub.api()
    #expect(try await api.health() == Health(ok: true))
    let version = try await api.version()
    #expect(version.api == 1)
    #expect(version.tools.contains("scout"))
  }

  @Test func a401AsksToPairAgain() async {
    stub.route { _ in .init(status: 401, body: Fixture.data("error-unauthorized")) }
    await #expect(throws: APIError.unauthorized) { try await stub.api().projects() }
    #expect(APIError.unauthorized.localizedDescription.contains("beagle serve pair"))
  }

  @Test func a404CarriesTheServersMessage() async throws {
    stub.serve(["/v1/scout/projects/no-such-project": "error-not-found"], status: 404)
    let error = await #expect(throws: APIError.self) {
      try await stub.api().project("no-such-project")
    }
    guard case .server(404, "not_found", let message) = error else {
      Issue.record("unexpected \(String(describing: error))")
      return
    }
    #expect(message.contains("no-such-project"))
  }

  @Test func aNonJSONErrorStillReads() {
    let error = APIError.from(status: 502, body: Data("Bad Gateway".utf8))
    #expect(error == .server(status: 502, code: nil, message: "Bad Gateway"))
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

  @Test func transportErrorsAreReadable() async {
    let api = BeagleAPI(
      server: URL(string: "https://unrouted.example.ts.net:8787")!, token: Fixture.token,
      session: stub.session)
    let error = await #expect(throws: APIError.self) { try await api.projects() }
    guard case .transport(let urlError) = error else {
      Issue.record("unexpected \(String(describing: error))")
      return
    }
    #expect(urlError.code == .cannotFindHost)
    #expect(error?.localizedDescription.contains("Tailscale") == true)
  }

  @Test func pathSegmentsArePercentEncoded() {
    let url = stub.api().url(
      ["v1", "scout", "projects", "a b"], query: [.init(name: "lines", value: "5")])
    #expect(url.absoluteString == "https://\(stub.host):8787/v1/scout/projects/a%20b?lines=5")
  }
}

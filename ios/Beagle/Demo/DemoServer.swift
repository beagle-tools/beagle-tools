#if DEBUG
  import BeaglePairing
  import Foundation

  /// A fake `beagle serve` inside the app, for UI tests and previews (Debug builds only).
  ///
  /// Launch with `-demo-server` to pair with it. Its answers follow docs/serve-api.md, with neutral
  /// example data.
  nonisolated final class DemoServer: URLProtocol {
    static let link: PairingLink = {
      do {
        return try PairingLink(
          server: "https://demo.example.ts.net:8787", token: String(repeating: "d", count: 64))
      } catch {
        preconditionFailure("the demo pairing link is invalid: \(error)")
      }
    }()

    static var session: URLSession {
      let config = URLSessionConfiguration.ephemeral
      config.protocolClasses = [DemoServer.self]
      return URLSession(configuration: config)
    }

    override class func canInit(with request: URLRequest) -> Bool {
      request.url?.host() == link.server.host()
    }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func stopLoading() {}

    override func startLoading() {
      let path = request.url?.path() ?? ""
      let authorized =
        path == "/v1/health"
        || request.value(forHTTPHeaderField: "Authorization") == "Bearer \(Self.link.token)"
      let (status, body): (Int, Any) =
        if !authorized {
          (401, DemoData.error("unauthorized", "missing or wrong bearer token"))
        } else if let answer = DemoDevices.answer(
          request.httpMethod ?? "GET", path, request.bodyData)
        {
          answer
        } else if let body = DemoData.routes[path] {
          (200, body)
        } else {
          (404, DemoData.error("not_found", "no route \(path)"))
        }
      let data = (try? JSONSerialization.data(withJSONObject: body)) ?? Data()
      guard let url = request.url,
        let response = HTTPURLResponse(
          url: url, statusCode: status, httpVersion: "HTTP/1.1",
          headerFields: ["Content-Type": "application/json; charset=utf-8"])
      else {
        client?.urlProtocol(self, didFailWithError: URLError(.badURL))
        return
      }
      client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
      client?.urlProtocol(self, didLoad: data)
      client?.urlProtocolDidFinishLoading(self)
    }
  }

  /// The demo store: one active project with a working task, one fresh one.
  nonisolated enum DemoData {
    static func error(_ code: String, _ message: String) -> [String: Any] {
      ["error": ["code": code, "message": message]]
    }

    static func stamp(minutesAgo: Double) -> String {
      Date(timeIntervalSinceNow: -minutesAgo * 60).formatted(.iso8601)
    }

    static let project = "rate-limit-the-api"
    static var tasks: [[String: Any]] {
      [
        task(1, "map-request-path", "Map the request path", "done", 4, 4),
        task(2, "add-allowlist", "Add the allowlist", "working", 1, 5),
        task(3, "load-test", "Load-test the limiter", "todo", 0, 2),
      ]
    }

    static func task(
      _ number: Int, _ slug: String, _ title: String, _ status: String, _ done: Int, _ total: Int
    ) -> [String: Any] {
      [
        "id": String(format: "%02d-%@", number, slug), "number": number, "slug": slug,
        "title": title, "status": status, "checklist": ["done": done, "total": total],
        "links": number == 2 ? ["https://github.com/acme/widgets/pull/42"] : [],
        "created": stamp(minutesAgo: 90), "updated": stamp(minutesAgo: Double(30 - number * 10)),
      ]
    }

    static func detail(_ index: Int, markdown: String, nextSteps: String?) -> [String: Any] {
      var out = tasks[index]
      out["project"] = project
      out["markdown"] = markdown
      out["next_steps"] = nextSteps ?? NSNull()
      return out
    }

    static let handoff = "Next is 02: add the allowlist table before the limiter."

    static var routes: [String: Any] {
      let base = "/v1/scout/projects"
      return [
        "/v1/health": ["ok": true],
        "/v1/version": ["beagle": "0.0.0-demo", "api": 1, "tools": ["scout"]],
        base: ["projects": [rateLimitRow, tidyRow], "warnings": []],
        "\(base)/\(project)": rateLimit,
        "\(base)/tidy-the-docs": tidy,
        "\(base)/\(project)/tasks/01-map-request-path": detail(
          0, markdown: task1, nextSteps: handoff),
        "\(base)/\(project)/tasks/02-add-allowlist": detail(1, markdown: task2, nextSteps: nil),
        "\(base)/\(project)/tasks/03-load-test": detail(2, markdown: task3, nextSteps: nil),
        "\(base)/\(project)/log": ["project": project, "lines": log, "truncated": false],
        "\(base)/tidy-the-docs/log": ["project": "tidy-the-docs", "lines": [], "truncated": false],
      ]
    }

    static let goal = "Every public endpoint answers 429 past 100 requests a minute per key."

    static var rateLimitRow: [String: Any] {
      [
        "id": project, "title": "Rate-limit the public API", "status": "active", "goal": goal,
        "tasks_closed": 1, "tasks_total": 3, "working": ["02-add-allowlist"], "links": ["ABC-123"],
        "created": stamp(minutesAgo: 90), "updated": stamp(minutesAgo: 20),
        "last_activity": stamp(minutesAgo: 2),
      ]
    }

    static var tidyRow: [String: Any] {
      [
        "id": "tidy-the-docs", "title": "Tidy the docs", "status": "planning", "goal": NSNull(),
        "tasks_closed": 0, "tasks_total": 0, "working": [], "links": [],
        "created": stamp(minutesAgo: 600), "updated": NSNull(),
        "last_activity": stamp(minutesAgo: 600),
      ]
    }

    static var rateLimit: [String: Any] {
      var out = rateLimitRow
      for key in ["tasks_closed", "tasks_total", "working"] { out[key] = nil }
      out["markdown"] = """
        ## Goal

        \(goal)

        ## How this part of the stack works

        ```text
        client ──▶ edge proxy ──▶ auth ──▶ limiter ──▶ handler
        ```

        ## Done when

        - [ ] A key past 100 requests a minute gets 429
        """
      out["tasks"] = tasks
      out["handoff"] = ["task": "01-map-request-path", "markdown": handoff]
      out["warnings"] = []
      return out
    }

    static var tidy: [String: Any] {
      var out = tidyRow
      for key in ["tasks_closed", "tasks_total", "working"] { out[key] = nil }
      out["markdown"] =
        "## Goal\n\n<One or two sentences: what will be true when this is finished.>\n"
      out["tasks"] = []
      out["handoff"] = NSNull()
      out["warnings"] = []
      return out
    }

    static let task1 = """
      ## Context

      Requests enter through the edge proxy and reach the **widgets** service.

      ## Steps

      - [x] Trace a request from the edge to the handler
      - [x] List the middleware in order
      - [x] Pick the spot for the limiter

      ## Done when

      - [x] The middleware order is written down

      ## Outcome

      The limiter goes right after auth, in `middleware/limit.rs`.

      ## Next steps

      \(handoff)
      """

    static let task2 = """
      ## Context

      Internal keys must never be limited.

      ## Objective

      An `allowlist` table and a lookup before the limiter.

      ## Steps

      - [x] Add the migration
      - [ ] Load the list at startup
      - [ ] Skip the limiter for listed keys
      - [ ] Test it

      ## Done when

      - [ ] A listed key never gets a 429

      ## Outcome

      <Filled in when the task closes.>
      """

    static let task3 = """
      ## Context

      <What the piece of the system this task touches is.>

      ## Steps

      - [ ] <step>
      """

    static var log: [String] {
      [
        "- **2026-11-24 09:00 UTC** project created: Rate-limit the public API",
        "- **2026-11-24 09:40 UTC** `01-map-request-path` working → done",
        "- **2026-11-24 09:41 UTC** `02-add-allowlist` todo → working",
        "- **2026-11-24 09:41 UTC** `02-add-allowlist` starting: add the allowlist table",
      ]
    }
  }
#endif

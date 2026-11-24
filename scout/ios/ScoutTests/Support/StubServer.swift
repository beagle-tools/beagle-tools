import Foundation

@testable import Scout

/// A fake `beagle serve` behind URLProtocol. Each test gets its own host, so
/// tests running in parallel never see each other's routes.
final class StubServer: Sendable {
  struct Reply: Sendable {
    let status: Int
    let body: Data
  }

  typealias Route = @Sendable (URLRequest) -> Reply

  let host: String
  let server: URL
  let session: URLSession

  init() {
    host = "mac-\(UUID().uuidString.lowercased()).example.ts.net"
    server = URL(string: "https://\(host):8787")!
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [StubProtocol.self]
    session = URLSession(configuration: config)
  }

  /// Answers every request with `route`.
  func route(_ route: @escaping Route) {
    StubProtocol.routes.set(host, route)
  }

  /// Answers `path` with a fixture file; anything else is a 404.
  func serve(_ files: [String: String], status: Int = 200) {
    route { request in
      guard let name = files[request.url?.path() ?? ""] else {
        return Reply(
          status: 404, body: Data(#"{"error":{"code":"not_found","message":"no route"}}"#.utf8))
      }
      return Reply(status: status, body: Fixture.data(name))
    }
  }

  func api(token: String? = Fixture.token) -> BeagleAPI {
    BeagleAPI(server: server, token: token, session: session)
  }
}

/// Routes by host, guarded by a lock (URLProtocol runs on its own threads).
final class Routes: @unchecked Sendable {
  private let lock = NSLock()
  private var routes: [String: StubServer.Route] = [:]

  func set(_ host: String, _ route: @escaping StubServer.Route) {
    lock.withLock { routes[host] = route }
  }

  func get(_ host: String) -> StubServer.Route? {
    lock.withLock { routes[host] }
  }
}

final class StubProtocol: URLProtocol {
  static let routes = Routes()

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

  override func startLoading() {
    guard let url = request.url, let route = Self.routes.get(url.host() ?? "") else {
      client?.urlProtocol(self, didFailWithError: URLError(.cannotFindHost))
      return
    }
    let reply = route(request)
    let response = HTTPURLResponse(
      url: url, statusCode: reply.status, httpVersion: "HTTP/1.1",
      headerFields: ["Content-Type": "application/json; charset=utf-8"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: reply.body)
    client?.urlProtocolDidFinishLoading(self)
  }

  override func stopLoading() {}
}

/// JSON captured from a real `beagle serve --no-tls` on loopback, reading a
/// neutral example scout store (ScoutTests/Fixtures).
enum Fixture {
  static let token = String(repeating: "0123456789abcdef", count: 4)

  private final class Token {}

  static func data(_ name: String) -> Data {
    let bundle = Bundle(for: Token.self)
    guard let url = bundle.url(forResource: name, withExtension: "json"),
      let data = try? Data(contentsOf: url)
    else { fatalError("missing fixture \(name).json") }
    return data
  }
}

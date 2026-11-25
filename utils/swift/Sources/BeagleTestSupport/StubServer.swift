import BeagleAPI
import Foundation

/// A fake `beagle serve` behind URLProtocol, for tests. Each instance gets its
/// own host, so tests running in parallel never see each other's routes.
public final class StubServer: Sendable {
  /// One HTTP answer.
  public struct Reply: Sendable {
    /// The HTTP status.
    public let status: Int
    /// The JSON body.
    public let body: Data

    /// An answer with `status` and `body`.
    public init(status: Int, body: Data) {
      self.status = status
      self.body = body
    }
  }

  /// Answers a request.
  public typealias Route = @Sendable (URLRequest) -> Reply

  /// This stub's unique host, e.g. `mac-<uuid>.example.ts.net`.
  public let host: String
  /// `https://<host>:8787`.
  public let server: URL
  /// A session whose requests this stub answers.
  public let session: URLSession

  /// The token tests pair with: 64 obviously fake hex characters.
  public static let token = String(repeating: "0123456789abcdef", count: 4)

  /// A stub with no routes (every request fails as an unknown host).
  public init() {
    host = "mac-\(UUID().uuidString.lowercased()).example.ts.net"
    var parts = URLComponents()
    parts.scheme = "https"
    parts.host = host
    parts.port = 8787
    guard let url = parts.url else { preconditionFailure("bad stub URL for \(host)") }
    server = url
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [StubProtocol.self]
    session = URLSession(configuration: config)
  }

  /// Answers every request with `route`.
  public func route(_ route: @escaping Route) {
    StubProtocol.routes.set(host, route)
  }

  /// Answers each path in `files` with its data; anything else is a 404.
  public func serve(_ files: [String: Data], status: Int = 200) {
    route { request in
      guard let body = files[request.url?.path() ?? ""] else {
        return Reply(
          status: 404, body: Data(#"{"error":{"code":"not_found","message":"no route"}}"#.utf8))
      }
      return Reply(status: status, body: body)
    }
  }

  /// A client for this stub, with `token`.
  public func api(token: String? = StubServer.token) -> BeagleAPI {
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

/// The URLProtocol behind ``StubServer``.
public final class StubProtocol: URLProtocol {
  static let routes = Routes()

  override public class func canInit(with request: URLRequest) -> Bool { true }
  override public class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

  override public func startLoading() {
    guard let url = request.url, let route = Self.routes.get(url.host() ?? "") else {
      client?.urlProtocol(self, didFailWithError: URLError(.cannotFindHost))
      return
    }
    let reply = route(request)
    guard
      let response = HTTPURLResponse(
        url: url, statusCode: reply.status, httpVersion: "HTTP/1.1",
        headerFields: ["Content-Type": "application/json; charset=utf-8"])
    else {
      client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
      return
    }
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: reply.body)
    client?.urlProtocolDidFinishLoading(self)
  }

  override public func stopLoading() {}
}

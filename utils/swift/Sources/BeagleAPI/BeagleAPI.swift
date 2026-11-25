import Foundation

/// A client for `beagle serve` (docs/serve-api.md, API v1): reads, plus the device writes push
/// notifications need.
///
/// The server-wide endpoints live here; each app adds its tool's endpoints
/// in an extension built on ``get(_:query:authorized:)``.
public struct BeagleAPI: Sendable {
  /// The server's origin, e.g. `https://mac.example.ts.net:8787`.
  public let server: URL
  /// The bearer token from pairing; `nil` only for unauthenticated probes.
  public let token: String?
  /// The session requests go through (a stub in tests).
  public let session: URLSession

  /// The API version this client speaks (`api` in `/v1/version`).
  public static let apiVersion = 1

  /// A client for `server`, sending `token` on every request but `/v1/health`.
  public init(server: URL, token: String?, session: URLSession = .shared) {
    self.server = server
    self.token = token
    self.session = session
  }

  /// `GET /v1/health`, without the token: is the server up?
  public func health() async throws -> Health {
    try await get(["v1", "health"], authorized: false)
  }

  /// `GET /v1/version`: the beagle version, API version and served tools.
  public func version() async throws -> ServerVersion {
    try await get(["v1", "version"])
  }

  /// The URL for a path under the server, each segment percent-encoded.
  public func url(_ segments: [String], query: [URLQueryItem] = []) -> URL {
    var url = server
    for segment in segments {
      url.append(path: segment, directoryHint: .notDirectory)
    }
    if !query.isEmpty {
      url.append(queryItems: query)
    }
    return url
  }

  /// The decoder for every answer: snake_case keys to camelCase properties.
  public static let decoder: JSONDecoder = {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    return decoder
  }()

  /// `GET`s a path and decodes the JSON answer; failures are ``APIError``s.
  public func get<T: Decodable>(
    _ segments: [String], query: [URLQueryItem] = [], authorized: Bool = true
  ) async throws -> T {
    let request = makeRequest("GET", url(segments, query: query), authorized: authorized)
    return try decode(try await perform(request), with: Self.decoder)
  }

  /// Sends a request with a JSON body and decodes the JSON answer.
  ///
  /// `body` is `nil` for no body; `decoder` defaults to ``decoder``. Failures are ``APIError``s.
  /// Only the push endpoints write (docs/serve-push.md).
  public func send<T: Decodable>(
    _ method: String, _ segments: [String], body: Data? = nil,
    decoder: JSONDecoder = BeagleAPI.decoder
  ) async throws -> T {
    try decode(try await send(method, segments, body: body), with: decoder)
  }

  /// Sends a request with a JSON body and returns the raw answer.
  ///
  /// For answers with no body, such as `204`.
  @discardableResult
  public func send(_ method: String, _ segments: [String], body: Data? = nil) async throws -> Data {
    var request = makeRequest(method, url(segments), authorized: true)
    if let body {
      request.httpBody = body
      request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    }
    return try await perform(request)
  }

  private func makeRequest(_ method: String, _ url: URL, authorized: Bool) -> URLRequest {
    var request = URLRequest(url: url)
    request.httpMethod = method
    request.timeoutInterval = 15
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    if authorized, let token {
      request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    }
    return request
  }

  /// Runs `request`; a non-2xx answer or a transport failure is an ``APIError``.
  private func perform(_ request: URLRequest) async throws -> Data {
    let data: Data
    let response: URLResponse
    do {
      (data, response) = try await session.data(for: request)
    } catch let error as URLError {
      throw APIError.transport(error)
    }
    guard let http = response as? HTTPURLResponse else { throw APIError.badResponse }
    guard (200..<300).contains(http.statusCode) else {
      throw APIError.from(status: http.statusCode, body: data)
    }
    return data
  }

  private func decode<T: Decodable>(_ data: Data, with decoder: JSONDecoder) throws -> T {
    do {
      return try decoder.decode(T.self, from: data)
    } catch {
      throw APIError.decoding(Self.describe(error))
    }
  }

  /// A short, readable reason a decode failed (which key, at what path).
  private static func describe(_ error: Error) -> String {
    guard let error = error as? DecodingError else { return String(describing: error) }
    func path(_ context: DecodingError.Context) -> String {
      context.codingPath.map { $0.intValue.map(String.init) ?? $0.stringValue }
        .joined(separator: ".")
    }
    switch error {
    case .keyNotFound(let key, let context):
      let at = path(context)
      return "missing `\(key.stringValue)`" + (at.isEmpty ? "" : " in `\(at)`")
    case .typeMismatch(_, let context), .valueNotFound(_, let context):
      return "unexpected value at `\(path(context))`"
    case .dataCorrupted(let context):
      return context.debugDescription
    @unknown default:
      return String(describing: error)
    }
  }
}

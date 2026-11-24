import Foundation

/// A read-only client for `beagle serve` (docs/serve-api.md, API v1).
nonisolated struct BeagleAPI: Sendable {
  /// The server's origin, e.g. `https://mac.example.ts.net:8787`.
  let server: URL
  /// The bearer token from pairing; `nil` only for unauthenticated probes.
  let token: String?
  let session: URLSession

  /// The API version this app speaks (`api` in `/v1/version`).
  static let apiVersion = 1

  init(server: URL, token: String?, session: URLSession = .shared) {
    self.server = server
    self.token = token
    self.session = session
  }

  func health() async throws -> Health {
    try await get(["v1", "health"], authorized: false)
  }

  func version() async throws -> ServerVersion {
    try await get(["v1", "version"])
  }

  func projects() async throws -> ProjectList {
    try await get(["v1", "scout", "projects"])
  }

  func project(_ id: String) async throws -> ProjectDetail {
    try await get(["v1", "scout", "projects", id])
  }

  /// `task` is a number (`2`), a full id (`02-add-allowlist`) or a slug.
  func task(project: String, task: String) async throws -> TaskDetail {
    try await get(["v1", "scout", "projects", project, "tasks", task])
  }

  /// The last `lines` lines of the project's log (the server caps it at 2000).
  func log(project: String, lines: Int = 200) async throws -> LogTail {
    try await get(
      ["v1", "scout", "projects", project, "log"],
      query: [URLQueryItem(name: "lines", value: String(lines))])
  }

  /// The URL for a path under the server, each segment percent-encoded.
  func url(_ segments: [String], query: [URLQueryItem] = []) -> URL {
    var url = server
    for segment in segments {
      url.append(path: segment, directoryHint: .notDirectory)
    }
    if !query.isEmpty {
      url.append(queryItems: query)
    }
    return url
  }

  private static let decoder: JSONDecoder = {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    return decoder
  }()

  private func get<T: Decodable>(
    _ segments: [String], query: [URLQueryItem] = [], authorized: Bool = true
  ) async throws -> T {
    var request = URLRequest(url: url(segments, query: query))
    request.timeoutInterval = 15
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    if authorized, let token {
      request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    }

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
    do {
      return try Self.decoder.decode(T.self, from: data)
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

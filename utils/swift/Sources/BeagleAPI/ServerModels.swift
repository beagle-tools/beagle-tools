import Foundation

// The server-wide JSON `beagle serve` answers (docs/serve-api.md). Each tool's
// own models live in its app.

/// `GET /v1/health`: answered without a token.
public struct Health: Decodable, Equatable, Sendable {
  /// Always true when the server answers.
  public let ok: Bool

  /// A health answer (for tests and previews).
  public init(ok: Bool) { self.ok = ok }
}

/// `GET /v1/version`.
public struct ServerVersion: Decodable, Equatable, Sendable {
  /// The beagle release, e.g. `0.7.0`.
  public let beagle: String
  /// The API version; changes only on a breaking change.
  public let api: Int
  /// The tools this server serves, e.g. `["scout", "rca"]`.
  public let tools: [String]

  /// A version answer (for tests and previews).
  public init(beagle: String, api: Int, tools: [String]) {
    self.beagle = beagle
    self.api = api
    self.tools = tools
  }
}

/// Every non-2xx body: `{ "error": { "code", "message" } }`.
public struct ErrorBody: Decodable, Equatable, Sendable {
  /// The error itself.
  public struct Detail: Decodable, Equatable, Sendable {
    /// Machine-readable, e.g. `not_found`.
    public let code: String
    /// For people, e.g. which project wasn't found.
    public let message: String
  }

  /// The error itself.
  public let error: Detail
}

/// RFC 3339 timestamps from the server.
public enum Timestamp {
  /// The date for an RFC 3339 string, or nil.
  public static func date(_ text: String?) -> Date? {
    guard let text else { return nil }
    return try? Date(text, strategy: .iso8601)
  }
}

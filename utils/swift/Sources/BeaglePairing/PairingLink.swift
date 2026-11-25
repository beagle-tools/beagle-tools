import Foundation

/// The server and token `beagle serve pair` hands the phone, as the QR code's
/// `beagle://pair?url=<percent-encoded origin>&token=<64 hex>` link or typed by hand.
public struct PairingLink: Equatable, Sendable {
  /// The server's origin, e.g. `https://mac.example.ts.net:8787` (no path).
  public let server: URL
  /// The 64-hex bearer token, lowercased.
  public let token: String

  /// Why a link or typed entry was rejected.
  public enum Problem: Error, Equatable, Sendable, LocalizedError {
    case notAPairingLink
    case missingServer
    case badServer(String)
    case insecureServer
    case missingToken
    case badToken

    /// A message for the person holding the phone.
    public var errorDescription: String? {
      switch self {
      case .notAPairingLink:
        return "That isn't a pairing link. It starts with beagle://pair (from `beagle serve pair`)."
      case .missingServer: return "The server address is missing."
      case .badServer(let text):
        return "“\(text)” isn't a server address like https://mac.example.ts.net:8787."
      case .insecureServer: return "The server address must start with https://."
      case .missingToken: return "The token is missing."
      case .badToken: return "The token should be 64 letters and digits (0–9, a–f)."
      }
    }
  }

  /// Parses a scanned or pasted `beagle://pair?…` link.
  ///
  /// Query values are percent-decoded.
  public static func parse(_ text: String) throws(Problem) -> PairingLink {
    let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
    guard let parts = URLComponents(string: trimmed),
      parts.scheme?.lowercased() == "beagle", parts.host?.lowercased() == "pair"
    else { throw .notAPairingLink }
    let items = parts.queryItems ?? []
    let server = items.first { $0.name == "url" }?.value ?? ""
    let token = items.first { $0.name == "token" }?.value ?? ""
    return try PairingLink(server: server, token: token)
  }

  /// Validates a server address and token entered by hand.
  public init(server text: String, token rawToken: String) throws(Problem) {
    var serverText = text.trimmingCharacters(in: .whitespacesAndNewlines)
    while serverText.hasSuffix("/") { serverText.removeLast() }
    guard !serverText.isEmpty else { throw .missingServer }
    guard let parts = URLComponents(string: serverText), let scheme = parts.scheme?.lowercased(),
      let host = parts.host, !host.isEmpty, parts.path.isEmpty, parts.query == nil,
      parts.fragment == nil, parts.user == nil
    else { throw .badServer(serverText) }
    // Plain http only for a server on this device (local testing, `--no-tls`).
    let loopback = ["127.0.0.1", "localhost", "::1"].contains(host.lowercased())
    guard scheme == "https" || (scheme == "http" && loopback) else {
      throw scheme == "http" ? .insecureServer : .badServer(serverText)
    }
    guard let url = parts.url else { throw .badServer(serverText) }

    let token = rawToken.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    guard !token.isEmpty else { throw .missingToken }
    guard token.count == 64, token.allSatisfy({ $0.isHexDigit }) else { throw .badToken }

    self.server = url
    self.token = token
  }

  /// The host shown in Settings, e.g. `mac.example.ts.net:8787`.
  public var displayName: String {
    guard let host = server.host() else { return server.absoluteString }
    return server.port.map { "\(host):\($0)" } ?? host
  }
}

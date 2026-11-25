import Foundation

/// Why a request to `beagle serve` failed, worded for the person holding the phone.
public enum APIError: Error, Equatable, Sendable, LocalizedError {
  /// 401: the token is missing, wrong or rotated away.
  case unauthorized
  /// Any other non-2xx answer, with the server's `{ "error": … }` when it sent one.
  case server(status: Int, code: String?, message: String)
  /// The request never got an HTTP answer (no network, Tailscale off, TLS failure…).
  case transport(URLError)
  /// The answer wasn't the JSON this app expects.
  case decoding(String)
  /// Not an HTTP response at all.
  case badResponse

  /// Builds the error for a non-2xx `status` from its body.
  public static func from(status: Int, body: Data) -> APIError {
    if status == 401 { return .unauthorized }
    if let parsed = try? JSONDecoder().decode(ErrorBody.self, from: body) {
      return .server(status: status, code: parsed.error.code, message: parsed.error.message)
    }
    let text = String(data: body.prefix(200), encoding: .utf8) ?? ""
    let message = text.isEmpty ? HTTPURLResponse.localizedString(forStatusCode: status) : text
    return .server(status: status, code: nil, message: message)
  }

  public var errorDescription: String? {
    switch self {
    case .unauthorized:
      return "Your Mac rejected this phone's token. Pair again: run `beagle serve pair` on the Mac."
    case .server(let status, _, let message):
      return "The server answered \(status): \(message)"
    case .transport(let error):
      return Self.describe(error)
    case .decoding(let detail):
      return "The server sent data this app doesn't understand (\(detail)). "
        + "Update the app or beagle so they match."
    case .badResponse:
      return "The server's answer wasn't HTTP."
    }
  }

  private static func describe(_ error: URLError) -> String {
    switch error.code {
    case .notConnectedToInternet, .networkConnectionLost, .dataNotAllowed:
      return "This phone is offline."
    case .cannotFindHost, .dnsLookupFailed:
      return "Can't find your Mac. Is Tailscale connected on this phone?"
    case .cannotConnectToHost:
      return "Your Mac refused the connection. Is `beagle serve` running?"
    case .timedOut:
      return "Your Mac didn't answer in time. Is it awake, with Tailscale and `beagle serve` on?"
    case .secureConnectionFailed, .serverCertificateUntrusted, .serverCertificateHasBadDate,
      .serverCertificateNotYetValid, .serverCertificateHasUnknownRoot,
      .clientCertificateRejected:
      return "The Mac's HTTPS certificate isn't trusted or has expired. "
        + "Renew it with `tailscale cert` and restart `beagle serve`."
    case .appTransportSecurityRequiresSecureConnection:
      return "The server address must use https://."
    default:
      return error.localizedDescription
    }
  }
}

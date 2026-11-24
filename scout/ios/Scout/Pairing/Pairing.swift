import Foundation
import Observation

/// The paired server: its address (UserDefaults) and token (Keychain).
/// Pairing checks the server before saving anything.
@Observable
final class Pairing {
  /// The paired server and token, or `nil` before pairing.
  private(set) var current: PairingLink?

  @ObservationIgnored private let keychain: Keychain
  @ObservationIgnored private let defaults: UserDefaults
  @ObservationIgnored private let session: URLSession

  static let serverKey = "pairing.server"
  static let tokenAccount = "beagle-serve-token"

  enum Problem: Error, Equatable, LocalizedError {
    case unsupportedAPI(Int)
    case noScout

    var errorDescription: String? {
      switch self {
      case .unsupportedAPI(let api):
        return "This server speaks API v\(api); this app needs v\(BeagleAPI.apiVersion). "
          + "Update the app or beagle."
      case .noScout:
        return "This server doesn't serve scout."
      }
    }
  }

  init(
    keychain: Keychain = Keychain(service: "dev.beagletools.scout"),
    defaults: UserDefaults = .standard,
    session: URLSession = .shared
  ) {
    self.keychain = keychain
    self.defaults = defaults
    self.session = session
    if let server = defaults.string(forKey: Self.serverKey),
      let token = try? keychain.read(Self.tokenAccount),
      let link = try? PairingLink(server: server, token: token)
    {
      current = link
    }
  }

  /// A client for the paired server, or `nil` before pairing.
  var api: BeagleAPI? {
    current.map { BeagleAPI(server: $0.server, token: $0.token, session: session) }
  }

  /// Checks the server answers (`/v1/health`, no token) and accepts the token
  /// (`/v1/version`), then saves the pairing. Nothing is saved on failure.
  @discardableResult
  func pair(_ link: PairingLink) async throws -> ServerVersion {
    let probe = BeagleAPI(server: link.server, token: link.token, session: session)
    _ = try await probe.health()
    let version = try await probe.version()
    guard version.api == BeagleAPI.apiVersion else { throw Problem.unsupportedAPI(version.api) }
    guard version.tools.contains("scout") else { throw Problem.noScout }
    try keychain.write(link.token, for: Self.tokenAccount)
    defaults.set(link.server.absoluteString, forKey: Self.serverKey)
    current = link
    return version
  }

  #if DEBUG
    /// Already paired with the in-app DemoServer, with throwaway storage.
    static func demo() -> Pairing {
      let suite = "dev.beagletools.scout.demo"
      let pairing = Pairing(
        keychain: Keychain(service: suite), defaults: UserDefaults(suiteName: suite) ?? .standard,
        session: DemoServer.session)
      pairing.current = DemoServer.link
      return pairing
    }
  #endif

  /// Forgets the server and deletes the token from the Keychain.
  func unpair() throws {
    try keychain.delete(Self.tokenAccount)
    defaults.removeObject(forKey: Self.serverKey)
    current = nil
  }
}

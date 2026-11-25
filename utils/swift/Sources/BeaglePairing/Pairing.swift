import BeagleAPI
import Foundation
import Observation

/// The Mac an app is paired with: its address (UserDefaults) and token
/// (Keychain). Pairing checks the server before saving anything.
@MainActor
@Observable
public final class Pairing {
  /// The paired server and token, or `nil` before pairing.
  public private(set) var current: PairingLink?

  /// The tool the app shows; pairing refuses a server that doesn't serve it.
  public let tool: String

  @ObservationIgnored private let secrets: any SecretStore
  @ObservationIgnored private let defaults: UserDefaults
  @ObservationIgnored private let session: URLSession

  /// The UserDefaults key holding the server URL.
  public static let serverKey = "pairing.server"
  /// The Keychain account holding the token.
  public static let tokenAccount = "beagle-serve-token"

  /// Why a reachable server with a good token was still refused.
  public enum Problem: Error, Equatable, LocalizedError {
    /// The server speaks a different API version.
    case unsupportedAPI(Int)
    /// The server doesn't serve this app's tool.
    case toolNotServed(String)

    public var errorDescription: String? {
      switch self {
      case .unsupportedAPI(let api):
        return "This server speaks API v\(api); this app needs v\(BeagleAPI.apiVersion). "
          + "Update the app or beagle."
      case .toolNotServed(let tool):
        return "This server doesn't serve \(tool)."
      }
    }
  }

  /// Loads any saved pairing for `tool` from `secrets` (the Keychain in an
  /// app) and `defaults`.
  public init(
    tool: String,
    secrets: any SecretStore,
    defaults: UserDefaults = .standard,
    session: URLSession = .shared
  ) {
    self.tool = tool
    self.secrets = secrets
    self.defaults = defaults
    self.session = session
    if let server = defaults.string(forKey: Self.serverKey),
      let token = try? secrets.read(Self.tokenAccount),
      let link = try? PairingLink(server: server, token: token)
    {
      current = link
    }
  }

  /// Already paired with `link`, saving nothing: for previews and demo servers.
  public init(tool: String, preview link: PairingLink, session: URLSession) {
    self.tool = tool
    self.secrets = MemorySecretStore()
    self.defaults = UserDefaults(suiteName: "dev.beagletools.preview") ?? .standard
    self.session = session
    current = link
  }

  /// A client for the paired server, or `nil` before pairing.
  public var api: BeagleAPI? {
    current.map { BeagleAPI(server: $0.server, token: $0.token, session: session) }
  }

  /// Checks the server answers (`/v1/health`, no token) and accepts the token
  /// (`/v1/version`, API v1, serving ``tool``), then saves the pairing.
  /// Nothing is saved on failure.
  @discardableResult
  public func pair(_ link: PairingLink) async throws -> ServerVersion {
    let probe = BeagleAPI(server: link.server, token: link.token, session: session)
    _ = try await probe.health()
    let version = try await probe.version()
    guard version.api == BeagleAPI.apiVersion else { throw Problem.unsupportedAPI(version.api) }
    guard version.tools.contains(tool) else { throw Problem.toolNotServed(tool) }
    try secrets.write(link.token, for: Self.tokenAccount)
    defaults.set(link.server.absoluteString, forKey: Self.serverKey)
    current = link
    return version
  }

  /// Forgets the server and deletes the token.
  public func unpair() throws {
    try secrets.delete(Self.tokenAccount)
    defaults.removeObject(forKey: Self.serverKey)
    current = nil
  }
}

import BeaglePairing
import Foundation

/// The id this install registers with `beagle serve` as: a random UUID made once and kept in the
/// Keychain, so it survives APNs token changes and app updates (docs/serve-push.md).
public enum InstallID {
  /// The Keychain account holding it.
  public static let account = "beagle-install-id"

  /// The saved id, or a new one saved now.
  ///
  /// Falls back to an unsaved one if the store fails, so
  /// push degrades to "registers again next launch" rather than not working.
  public static func load(from secrets: any SecretStore) -> String {
    if let saved = try? secrets.read(account), UUID(uuidString: saved) != nil {
      return saved
    }
    let fresh = UUID().uuidString.lowercased()
    try? secrets.write(fresh, for: account)
    return fresh
  }
}

extension Data {
  /// The bytes as lowercase hex: how APNs device tokens travel.
  public var hexString: String {
    map { String(format: "%02x", $0) }.joined()
  }
}

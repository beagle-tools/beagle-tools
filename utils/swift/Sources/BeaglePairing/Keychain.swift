import Foundation
import Security

/// Small wrapper over the iOS Keychain for string secrets (generic passwords).
///
/// Items stay on this device and are readable after its first unlock.
public struct Keychain: SecretStore {
  /// The Keychain service the items are filed under.
  public let service: String

  /// A Keychain for `service`, e.g. the app's bundle ID.
  public init(service: String) { self.service = service }

  /// A Keychain call failed with `status`.
  public struct Failure: Error, Equatable, LocalizedError {
    /// The Security framework status code.
    public let status: OSStatus
    /// A message for the person holding the phone.
    public var errorDescription: String? {
      let reason = SecCopyErrorMessageString(status, nil) as String? ?? "status \(status)"
      return "Keychain error: \(reason)"
    }
  }

  private func query(_ account: String) -> [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: service,
      kSecAttrAccount as String: account,
    ]
  }

  /// The value stored for `account`, or nil.
  public func read(_ account: String) throws -> String? {
    var q = query(account)
    q[kSecReturnData as String] = true
    q[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: CFTypeRef?
    let status = SecItemCopyMatching(q as CFDictionary, &result)
    if status == errSecItemNotFound { return nil }
    guard status == errSecSuccess, let data = result as? Data else { throw Failure(status: status) }
    return String(data: data, encoding: .utf8)
  }

  /// Stores `value` for `account`, replacing any old value.
  public func write(_ value: String, for account: String) throws {
    let data = Data(value.utf8)
    let update: [String: Any] = [kSecValueData as String: data]
    let status = SecItemUpdate(query(account) as CFDictionary, update as CFDictionary)
    if status == errSecSuccess { return }
    guard status == errSecItemNotFound else { throw Failure(status: status) }
    var add = query(account)
    add[kSecValueData as String] = data
    add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
    let added = SecItemAdd(add as CFDictionary, nil)
    guard added == errSecSuccess else { throw Failure(status: added) }
  }

  /// Removes the value stored for `account`; a missing one is fine.
  public func delete(_ account: String) throws {
    let status = SecItemDelete(query(account) as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else {
      throw Failure(status: status)
    }
  }
}

import Foundation

/// Where a pairing keeps its token. Apps use ``Keychain``; tests and previews
/// use ``MemorySecretStore`` (a package test bundle has no Keychain access in
/// the simulator).
public protocol SecretStore: Sendable {
  /// The value stored for `account`, or nil.
  func read(_ account: String) throws -> String?
  /// Stores `value` for `account`, replacing any old value.
  func write(_ value: String, for account: String) throws
  /// Removes `account`'s value; missing is fine.
  func delete(_ account: String) throws
}

/// Secrets in memory only, gone when the process ends.
public final class MemorySecretStore: SecretStore, @unchecked Sendable {
  private let lock = NSLock()
  private var values: [String: String] = [:]

  /// An empty store.
  public init() {}

  public func read(_ account: String) throws -> String? {
    lock.withLock { values[account] }
  }

  public func write(_ value: String, for account: String) throws {
    lock.withLock { values[account] = value }
  }

  public func delete(_ account: String) throws {
    _ = lock.withLock { values.removeValue(forKey: account) }
  }
}

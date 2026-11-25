import BeaglePairing
import Foundation
import Testing

/// BeagleKit's Keychain against the real iOS Keychain.
///
/// Runs here because the app hosts these tests; a package test bundle has no Keychain access.
struct KeychainTests {
  let keychain = Keychain(service: "dev.beagletools.scout.tests.\(UUID().uuidString)")

  @Test func overwritesReadsAndDeletes() throws {
    defer { try? keychain.delete("test") }
    #expect(try keychain.read("test") == nil)
    try keychain.write("one", for: "test")
    try keychain.write("two", for: "test")
    #expect(try keychain.read("test") == "two")
    try keychain.delete("test")
    try keychain.delete("test")
    #expect(try keychain.read("test") == nil)
  }

  @Test @MainActor func aPairingSurvivesARestartThroughTheKeychain() throws {
    let suite = "dev.beagletools.scout.tests.\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suite))
    defer {
      try? keychain.delete(Pairing.tokenAccount)
      defaults.removeObject(forKey: Pairing.serverKey)
    }
    let link = try PairingLink(
      server: "https://mac.example.ts.net:8787", token: String(repeating: "ab", count: 32))
    try keychain.write(link.token, for: Pairing.tokenAccount)
    defaults.set(link.server.absoluteString, forKey: Pairing.serverKey)
    #expect(Pairing(tool: "scout", secrets: keychain, defaults: defaults).current == link)
  }
}

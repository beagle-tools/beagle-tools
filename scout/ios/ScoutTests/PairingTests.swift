import Foundation
import Testing

@testable import Scout

/// Pairing against a stubbed server, with a throwaway Keychain service and
/// defaults suite per test.
@MainActor
@Suite(.serialized)
struct PairingTests {
  let stub = StubServer()
  let keychain = Keychain(service: "dev.beagletools.scout.tests.\(UUID().uuidString)")
  let suite = "dev.beagletools.scout.tests.\(UUID().uuidString)"

  // One defaults instance per test, created once: CFPrefs in the simulator
  // has crashed when many suite instances are created and wiped in parallel.
  let defaults: UserDefaults

  init() {
    defaults = UserDefaults(suiteName: suite)!
  }
  var link: PairingLink {
    try! PairingLink(server: stub.server.absoluteString, token: Fixture.token)
  }

  func makePairing() -> Pairing {
    Pairing(keychain: keychain, defaults: defaults, session: stub.session)
  }

  func cleanUp() {
    try? keychain.delete(Pairing.tokenAccount)
    defaults.removeObject(forKey: Pairing.serverKey)
  }

  @Test func pairingSavesTheTokenInTheKeychainAndSurvivesARestart() async throws {
    defer { cleanUp() }
    stub.serve(["/v1/health": "health", "/v1/version": "version"])
    let pairing = makePairing()
    #expect(pairing.current == nil)

    let version = try await pairing.pair(link)
    #expect(version.beagle.isEmpty == false)
    #expect(pairing.current == link)
    #expect(try keychain.read(Pairing.tokenAccount) == Fixture.token)
    // The token never goes to UserDefaults.
    #expect(defaults.string(forKey: Pairing.serverKey) == stub.server.absoluteString)
    #expect(
      defaults.dictionaryRepresentation().values.contains { "\($0)" == Fixture.token } == false)

    let restarted = makePairing()
    #expect(restarted.current == link)
    #expect(restarted.api?.token == Fixture.token)
  }

  @Test func aRejectedTokenSavesNothing() async {
    defer { cleanUp() }
    stub.route { request in
      request.url?.path() == "/v1/health"
        ? .init(status: 200, body: Fixture.data("health"))
        : .init(status: 401, body: Fixture.data("error-unauthorized"))
    }
    let pairing = makePairing()
    await #expect(throws: APIError.unauthorized) { try await pairing.pair(link) }
    #expect(pairing.current == nil)
    #expect((try? keychain.read(Pairing.tokenAccount)) == nil)
  }

  @Test func aNewerAPIIsRefused() async {
    defer { cleanUp() }
    stub.route { request in
      request.url?.path() == "/v1/health"
        ? .init(status: 200, body: Fixture.data("health"))
        : .init(status: 200, body: Data(#"{"beagle":"9.0.0","api":2,"tools":["scout"]}"#.utf8))
    }
    await #expect(throws: Pairing.Problem.unsupportedAPI(2)) { try await makePairing().pair(link) }
  }

  @Test func unpairForgetsEverything() async throws {
    defer { cleanUp() }
    stub.serve(["/v1/health": "health", "/v1/version": "version"])
    let pairing = makePairing()
    try await pairing.pair(link)
    try pairing.unpair()
    #expect(pairing.current == nil)
    #expect(try keychain.read(Pairing.tokenAccount) == nil)
    #expect(makePairing().current == nil)
  }

  @Test func keychainOverwritesAndDeletes() throws {
    defer { cleanUp() }
    try keychain.write("one", for: "test")
    try keychain.write("two", for: "test")
    #expect(try keychain.read("test") == "two")
    try keychain.delete("test")
    try keychain.delete("test")
    #expect(try keychain.read("test") == nil)
  }
}

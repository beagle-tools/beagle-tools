import BeagleAPI
import BeagleTestSupport
import Foundation
import Testing

@testable import BeaglePairing

/// Pairing against a stubbed server, with an in-memory secret store and a throwaway defaults suite
/// per test.
///
/// (The real Keychain is tested in the app, whose tests are hosted and may use it.)
@MainActor
@Suite(.serialized)
struct PairingTests {
  let stub = StubServer()
  let keychain = MemorySecretStore()
  let suite = "dev.beagletools.scout.tests.\(UUID().uuidString)"

  // One defaults instance per test, created once: CFPrefs in the simulator
  // has crashed when many suite instances are created and wiped in parallel.
  let defaults: UserDefaults
  let link: PairingLink

  init() throws {
    defaults = try #require(UserDefaults(suiteName: suite))
    link = try PairingLink(server: stub.server.absoluteString, token: StubServer.token)
  }

  func makePairing() -> Pairing {
    Pairing(tool: "scout", secrets: keychain, defaults: defaults, session: stub.session)
  }

  func cleanUp() {
    try? keychain.delete(Pairing.tokenAccount)
    defaults.removeObject(forKey: Pairing.serverKey)
  }

  @Test func pairingSavesTheTokenAndSurvivesARestart() async throws {
    defer { cleanUp() }
    stub.serve(["/v1/health": fixture("health"), "/v1/version": fixture("version")])
    let pairing = makePairing()
    #expect(pairing.current == nil)

    let version = try await pairing.pair(link)
    #expect(version.beagle.isEmpty == false)
    #expect(pairing.current == link)
    #expect(try keychain.read(Pairing.tokenAccount) == StubServer.token)
    // The token never goes to UserDefaults.
    #expect(defaults.string(forKey: Pairing.serverKey) == stub.server.absoluteString)
    #expect(
      defaults.dictionaryRepresentation().values.contains { "\($0)" == StubServer.token } == false)

    let restarted = makePairing()
    #expect(restarted.current == link)
    #expect(restarted.api?.token == StubServer.token)
  }

  @Test func aRejectedTokenSavesNothing() async {
    defer { cleanUp() }
    stub.route { request in
      request.url?.path() == "/v1/health"
        ? .init(status: 200, body: fixture("health"))
        : .init(status: 401, body: fixture("error-unauthorized"))
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
        ? .init(status: 200, body: fixture("health"))
        : .init(status: 200, body: Data(#"{"beagle":"9.0.0","api":2,"tools":["scout"]}"#.utf8))
    }
    await #expect(throws: Pairing.Problem.unsupportedAPI(2)) { try await makePairing().pair(link) }
  }

  @Test func unpairForgetsEverything() async throws {
    defer { cleanUp() }
    stub.serve(["/v1/health": fixture("health"), "/v1/version": fixture("version")])
    let pairing = makePairing()
    try await pairing.pair(link)
    try pairing.unpair()
    #expect(pairing.current == nil)
    #expect(try keychain.read(Pairing.tokenAccount) == nil)
    #expect(makePairing().current == nil)
  }

  @Test func aServerWithoutTheToolIsRefused() async {
    defer { cleanUp() }
    stub.route { request in
      request.url?.path() == "/v1/health"
        ? .init(status: 200, body: fixture("health"))
        : .init(status: 200, body: Data(#"{"beagle":"1.0.0","api":1,"tools":["rca"]}"#.utf8))
    }
    await #expect(throws: Pairing.Problem.toolNotServed("scout")) {
      try await makePairing().pair(link)
    }
  }

}

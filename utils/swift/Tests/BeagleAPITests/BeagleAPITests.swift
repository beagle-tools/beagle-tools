import BeagleTestSupport
import Foundation
import Testing

@testable import BeagleAPI

/// JSON captured from a real `beagle serve --no-tls` on loopback (Fixtures/).
func fixture(_ name: String) -> Data {
  guard let url = Bundle.module.url(forResource: "Fixtures/\(name)", withExtension: "json"),
    let data = try? Data(contentsOf: url)
  else {
    Issue.record("missing fixture \(name).json")
    return Data()
  }
  return data
}

struct BeagleAPITests {
  let stub = StubServer()

  @Test func healthNeedsNoTokenAndEverythingElseSendsIt() async throws {
    stub.route { request in
      let auth = request.value(forHTTPHeaderField: "Authorization")
      switch request.url?.path() {
      case "/v1/health":
        #expect(auth == nil)
        return .init(status: 200, body: fixture("health"))
      default:
        #expect(auth == "Bearer \(StubServer.token)")
        return .init(status: 200, body: fixture("version"))
      }
    }
    let api = stub.api()
    #expect(try await api.health() == Health(ok: true))
    let version = try await api.version()
    #expect(version.api == 1)
    #expect(version.tools.contains("scout"))
  }

  @Test func a401AsksToPairAgain() async {
    stub.route { _ in .init(status: 401, body: fixture("error-unauthorized")) }
    await #expect(throws: APIError.unauthorized) { try await stub.api().version() }
    #expect(APIError.unauthorized.localizedDescription.contains("beagle serve pair"))
  }

  @Test func a404CarriesTheServersMessage() async {
    stub.route { _ in .init(status: 404, body: fixture("error-not-found")) }
    let error = await #expect(throws: APIError.self) { try await stub.api().version() }
    guard case .server(404, "not_found", let message) = error else {
      Issue.record("unexpected \(String(describing: error))")
      return
    }
    #expect(message.contains("no-such-project"))
  }

  @Test func aNonJSONErrorStillReads() {
    let error = APIError.from(status: 502, body: Data("Bad Gateway".utf8))
    #expect(error == .server(status: 502, code: nil, message: "Bad Gateway"))
  }

  @Test func unexpectedJSONIsADecodingError() async {
    stub.route { _ in .init(status: 200, body: Data(#"{"beagle": 3}"#.utf8)) }
    let error = await #expect(throws: APIError.self) { try await stub.api().version() }
    guard case .decoding(let detail) = error else {
      Issue.record("unexpected \(String(describing: error))")
      return
    }
    #expect(detail.contains("beagle"))
  }

  @Test func transportErrorsAreReadable() async throws {
    let server = try #require(URL(string: "https://unrouted.example.ts.net:8787"))
    let api = BeagleAPI(server: server, token: StubServer.token, session: stub.session)
    let error = await #expect(throws: APIError.self) { try await api.version() }
    guard case .transport(let urlError) = error else {
      Issue.record("unexpected \(String(describing: error))")
      return
    }
    #expect(urlError.code == .cannotFindHost)
    #expect(error?.localizedDescription.contains("Tailscale") == true)
  }

  @Test func pathSegmentsArePercentEncoded() {
    let url = stub.api().url(["v1", "scout", "a b"], query: [.init(name: "lines", value: "5")])
    #expect(url.absoluteString == "https://\(stub.host):8787/v1/scout/a%20b?lines=5")
  }

  @Test func timestampsParseRFC3339() {
    #expect(Timestamp.date("2026-11-24T21:28:31Z") != nil)
    #expect(Timestamp.date("yesterday") == nil)
    #expect(Timestamp.date(nil) == nil)
  }
}

import BeagleAPI
import BeaglePairing
import BeagleTestSupport
import Foundation
import Testing

@testable import BeaglePush

/// The settings document as docs/serve-api.md shows it (scout's catalog and defaults).
let document = Data(
  """
  {"settings":{"scout":{"enabled":true,"events":{"blocked":true,"project_done":true,
  "project_started":true,"task_done":false,"task_paused":true,"task_started":false}}},
  "catalog":[{"tool":"scout","name":"Scout","events":[
  {"kind":"blocked","label":"A task is blocked (needs you)","default":true,"sound":true},
  {"kind":"project_started","label":"A project starts","default":true,"sound":false},
  {"kind":"project_done","label":"A project is done","default":true,"sound":false},
  {"kind":"task_done","label":"Every finished task","default":false,"sound":false},
  {"kind":"task_started","label":"Every task an agent picks up (noisy)","default":false,"sound":false},
  {"kind":"task_paused","label":"A working task went quiet","default":true,"sound":false}]}]}
  """.utf8)

/// A request's body: URLProtocol sees it as a stream, never `httpBody`.
func body(of request: URLRequest) -> Data {
  if let data = request.httpBody { return data }
  guard let stream = request.httpBodyStream else { return Data() }
  stream.open()
  defer { stream.close() }
  var data = Data()
  var buffer = [UInt8](repeating: 0, count: 4096)
  while stream.hasBytesAvailable {
    let read = stream.read(&buffer, maxLength: buffer.count)
    if read <= 0 { break }
    data.append(buffer, count: read)
  }
  return data
}

@MainActor
final class FakeAuthorizer: PushAuthorizer {
  var current = PushPermission.notDetermined
  var grant = true
  var registrations = 0

  func permission() async -> PushPermission { current }
  func requestPermission() async throws -> Bool {
    current = grant ? .authorized : .denied
    return grant
  }
  func registerForRemoteNotifications() { registrations += 1 }
}

/// Requests the stub saw, as "METHOD path" plus the last body.
final class Seen: @unchecked Sendable {
  private let lock = NSLock()
  private var lines: [String] = []
  private var bodies: [Data] = []
  func add(_ request: URLRequest) {
    lock.withLock {
      lines.append("\(request.httpMethod ?? "?") \(request.url?.path() ?? "")")
      bodies.append(body(of: request))
    }
  }
  var requests: [String] { lock.withLock { lines } }
  var lastBody: Data { lock.withLock { bodies.last ?? Data() } }
}

@MainActor
struct PushTests {
  let stub = StubServer()
  let seen = Seen()
  let authorizer = FakeAuthorizer()

  func makePush(paired: Bool = true) throws -> Push {
    stub.route { [seen] request in
      seen.add(request)
      if request.httpMethod == "DELETE" { return .init(status: 204, body: Data()) }
      if request.url?.path().hasSuffix("/test") == true {
        return .init(
          status: 503,
          body: Data(#"{"error":{"code":"push_not_configured","message":"no [push]"}}"#.utf8))
      }
      return .init(status: 200, body: document)
    }
    let link = try PairingLink(server: stub.server.absoluteString, token: StubServer.token)
    let pairing = Pairing(tool: "scout", preview: link, session: stub.session)
    if !paired { try pairing.unpair() }
    return Push(
      pairing: pairing, secrets: MemorySecretStore(), authorizer: authorizer,
      environment: "development", topic: "dev.beagletools.beagle", name: "iPhone")
  }

  @Test func neverPromptsOnLaunchAndAsksForATokenOnceAllowed() async throws {
    let push = try makePush()
    await push.start()
    #expect(push.permission == .notDetermined)
    #expect(authorizer.registrations == 0)

    await push.requestPermission()
    #expect(push.permission == .authorized)
    #expect(authorizer.registrations == 1)
  }

  @Test func aTokenRegistersThisPhoneAndShowsItsSettings() async throws {
    authorizer.current = .authorized
    let push = try makePush()
    await push.start()
    await push.sync()
    #expect(push.state == .waitingForToken)

    await push.didRegister(deviceToken: Data([0xab, 0x01, 0xff]))
    #expect(seen.requests == ["PUT /v1/devices/\(push.deviceID)"])
    let sent = try JSONSerialization.jsonObject(with: seen.lastBody) as? [String: String]
    #expect(
      sent == [
        "apns_token": "ab01ff", "environment": "development",
        "topic": "dev.beagletools.beagle", "name": "iPhone",
      ])
    guard case .registered(let doc) = push.state else {
      Issue.record("not registered: \(push.state)")
      return
    }
    #expect(doc.catalog.first?.events.map(\.kind).count == 6)
    #expect(doc.settings["scout"]?.events["task_done"] == false)
  }

  @Test func aToggleSavesTheWholeSettingsObject() async throws {
    authorizer.current = .authorized
    let push = try makePush()
    await push.start()
    await push.didRegister(deviceToken: Data([1, 2, 3]))
    guard case .registered(let doc) = push.state, let scout = doc.catalog.first else {
      Issue.record("not registered")
      return
    }
    await push.setEvent("task_done", true, for: scout)
    #expect(seen.requests.last == "PUT /v1/devices/\(push.deviceID)/notifications")
    let sent = try JSONDecoder().decode(
      [String: ToolNotificationSettings].self, from: seen.lastBody)
    #expect(sent["scout"]?.events["task_done"] == true)
    #expect(sent["scout"]?.events["blocked"] == true)
    #expect(sent["scout"]?.enabled == true)
  }

  @Test func withoutAPairingNothingIsSent() async throws {
    authorizer.current = .authorized
    let push = try makePush(paired: false)
    await push.start()
    await push.didRegister(deviceToken: Data([1]))
    #expect(push.state == .idle)
    #expect(seen.requests.isEmpty)
  }

  @Test func forgetDeletesThisPhoneAndTestSaysWhyItFailed() async throws {
    let push = try makePush()
    #expect(await push.sendTest().contains("no [push]"))
    await push.forget()
    #expect(seen.requests.last == "DELETE /v1/devices/\(push.deviceID)")
    #expect(push.state == .idle)
  }

  @Test func theInstallIDIsMadeOnceAndKept() {
    let secrets = MemorySecretStore()
    let first = InstallID.load(from: secrets)
    #expect(UUID(uuidString: first) != nil)
    #expect(InstallID.load(from: secrets) == first)
  }

  @Test func missingSettingsFallBackToTheCatalogDefaults() throws {
    var doc = try BeagleAPI.plainDecoder.decode(NotificationDocument.self, from: document)
    doc.settings = [:]
    let scout = try #require(doc.catalog.first)
    let defaults = doc.settings(for: scout)
    #expect(defaults.enabled)
    #expect(defaults.events["blocked"] == true)
    #expect(defaults.events["task_started"] == false)
  }
}

import BeagleAPI
import BeaglePairing
import BeaglePush
import BeagleTestSupport
import SwiftUI
import Testing
import UIKit

@testable import BeagleUI

/// Notifications already allowed; registering for a token does nothing.
@MainActor
final class GrantedAuthorizer: PushAuthorizer {
  func permission() async -> PushPermission { .authorized }
  func requestPermission() async throws -> Bool { true }
  func registerForRemoteNotifications() {}
}

@MainActor
struct NotificationSettingsSectionTests {
  let stub = StubServer()

  /// A Push registered with a stub Mac that answers part of scout's catalog and no saved settings.
  func registeredPush() async throws -> Push {
    let catalog = """
      {"settings":{},"catalog":[{"tool":"scout","name":"Scout","events":[
      {"kind":"blocked","label":"A task is blocked (needs you)","default":true,"sound":true},
      {"kind":"task_done","label":"Every finished task","default":false,"sound":false}]}]}
      """
    stub.route { _ in .init(status: 200, body: Data(catalog.utf8)) }
    let link = try PairingLink(server: stub.server.absoluteString, token: StubServer.token)
    let push = Push(
      pairing: Pairing(tool: "scout", preview: link, session: stub.session),
      secrets: MemorySecretStore(), authorizer: GrantedAuthorizer(),
      environment: "development", topic: "dev.beagletools.beagle", name: "iPhone")
    await push.start()
    await push.didRegister(deviceToken: Data([1, 2, 3]))
    return push
  }

  /// Lays the section out in a window, so SwiftUI evaluates its rows.
  func render(_ push: Push) {
    let host = UIHostingController(
      rootView: Form { NotificationSettingsSection() }.environment(push))
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 1200))
    window.rootViewController = host
    window.isHidden = false
    host.view.layoutIfNeeded()
    window.isHidden = true
  }

  @Test func drawsEachToolFromTheCatalogWithItsDefaults() async throws {
    let push = try await registeredPush()
    guard case .registered(let document) = push.state, let scout = document.catalog.first else {
      Issue.record("not registered: \(push.state)")
      return
    }
    let section = ToolNotificationsSection(tool: scout, settings: document.settings(for: scout))
    #expect(section.settings.events == ["blocked": true, "task_done": false])
    render(push)
  }
}

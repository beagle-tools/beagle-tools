import BeagleAPI
import BeaglePairing
import Foundation
import Observation

/// This phone's push notifications: permission, the APNs token, its registration with the paired
/// Mac, and its per-tool settings there (docs/serve-push.md, "The app").
///
/// Nothing here blocks the app: a failure shows on the Notifications screen and is retried on the
/// next launch or pairing.
@MainActor
@Observable
public final class Push {
  /// Where registration with the Mac stands.
  public enum State: Equatable, Sendable {
    /// Not paired, or notifications not allowed: nothing to register.
    case idle
    /// Allowed, waiting for APNs to hand over a token.
    case waitingForToken
    /// Sending the token to the Mac.
    case registering
    /// Registered; the Mac's settings and catalog for this phone.
    case registered(NotificationDocument)
    /// Registering failed; why, for the person holding the phone.
    case failed(String)
  }

  /// Whether this app may show notifications.
  public private(set) var permission: PushPermission = .notDetermined
  /// Where registration stands.
  public private(set) var state: State = .idle
  /// The last failed settings change or test push, if any.
  public private(set) var problem: String?

  /// The id this install registers as (``InstallID``).
  public let deviceID: String

  @ObservationIgnored private let pairing: Pairing
  @ObservationIgnored private let authorizer: any PushAuthorizer
  @ObservationIgnored private let environment: String
  @ObservationIgnored private let topic: String
  @ObservationIgnored private let name: String
  @ObservationIgnored private var token: String?

  /// Push for the Mac `pairing` points at. `environment` is `development` or `production` (the
  /// APNs host this build's tokens belong to), `topic` the bundle ID, `name` the phone's name.
  public init(
    pairing: Pairing, secrets: any SecretStore, authorizer: any PushAuthorizer,
    environment: String, topic: String, name: String
  ) {
    self.pairing = pairing
    self.authorizer = authorizer
    self.environment = environment
    self.topic = topic
    self.name = String(name.prefix(64))
    deviceID = InstallID.load(from: secrets)
  }

  /// On launch: reads the permission and, when allowed, asks APNs for a token.
  ///
  /// Never prompts.
  public func start() async {
    permission = await authorizer.permission()
    if permission == .authorized { authorizer.registerForRemoteNotifications() }
  }

  /// Shows the system prompt (from "Turn on notifications"), then registers when granted.
  public func requestPermission() async {
    do {
      _ = try await authorizer.requestPermission()
    } catch {
      problem = error.localizedDescription
    }
    await start()
  }

  /// APNs answered with this phone's token: send it to the Mac.
  public func didRegister(deviceToken: Data) async {
    token = deviceToken.hexString
    await sync()
  }

  /// APNs refused to give a token (for example, no Push capability in this build's profile).
  public func didFailToRegister(_ error: any Error) {
    state = .failed("This phone couldn't get a push token: \(error.localizedDescription)")
  }

  /// Registers the token with the paired Mac (on launch, a new token, or a new pairing).
  public func sync() async {
    guard permission == .authorized, let api = pairing.api else {
      state = .idle
      return
    }
    guard let token else {
      state = .waitingForToken
      return
    }
    state = .registering
    let registration = DeviceRegistration(
      apnsToken: token, environment: environment, topic: topic, name: name)
    do {
      state = .registered(try await api.registerDevice(deviceID, registration))
    } catch {
      state = .failed(error.localizedDescription)
    }
  }

  /// Reloads this phone's settings from the Mac (when the Notifications screen opens).
  ///
  /// A Mac that forgot this phone (`pair --rotate`) gets it registered again.
  public func refresh() async {
    guard case .registered = state, let api = pairing.api else { return }
    do {
      state = .registered(try await api.notificationSettings(deviceID))
    } catch APIError.server(status: 404, _, _) {
      await sync()
    } catch {
      state = .failed(error.localizedDescription)
    }
  }

  /// Turns a tool's notifications on or off as a whole.
  public func setEnabled(_ on: Bool, for tool: NotificationToolInfo) async {
    await change(tool) { $0.enabled = on }
  }

  /// Turns one of a tool's events on or off.
  public func setEvent(_ kind: String, _ on: Bool, for tool: NotificationToolInfo) async {
    await change(tool) { $0.events[kind] = on }
  }

  /// Saves a change to a tool's settings right away; the screen shows what the Mac saved.
  private func change(
    _ tool: NotificationToolInfo, _ edit: (inout ToolNotificationSettings) -> Void
  ) async {
    guard case .registered(var document) = state, let api = pairing.api else { return }
    var settings = document.settings(for: tool)
    edit(&settings)
    document.settings[tool.tool] = settings
    state = .registered(document)
    do {
      state = .registered(try await api.saveNotificationSettings(deviceID, document.settings))
      problem = nil
    } catch {
      problem = "Not saved: \(error.localizedDescription)"
      await sync()
    }
  }

  /// Asks the Mac to push a test notification now; answers what happened, in words.
  public func sendTest() async -> String {
    guard let api = pairing.api else { return "Pair with your Mac first." }
    do {
      return try await api.sendTestPush(deviceID).message
    } catch {
      return error.localizedDescription
    }
  }

  /// Before unpairing: the Mac forgets this phone, so it stops pushing to it.
  ///
  /// Best effort.
  public func forget() async {
    if let api = pairing.api { try? await api.forgetDevice(deviceID) }
    state = .idle
  }
}

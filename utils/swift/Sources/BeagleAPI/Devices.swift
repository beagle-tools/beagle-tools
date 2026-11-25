import Foundation

/// A phone registering for push notifications (`PUT /v1/devices/{device}`).
public struct DeviceRegistration: Codable, Equatable, Sendable {
  /// The APNs device token, as lowercase hex.
  public var apnsToken: String
  /// `development` (Xcode installs, the APNs sandbox) or `production`.
  public var environment: String
  /// The app's bundle ID, which APNs needs as `apns-topic`.
  public var topic: String
  /// The phone's name, shown nowhere but the Mac's device list.
  public var name: String

  /// A registration with every field.
  public init(apnsToken: String, environment: String, topic: String, name: String) {
    self.apnsToken = apnsToken
    self.environment = environment
    self.topic = topic
    self.name = name
  }

  enum CodingKeys: String, CodingKey {
    case apnsToken = "apns_token"
    case environment, topic, name
  }
}

/// One tool's notification switches: a master switch and one per event kind.
public struct ToolNotificationSettings: Codable, Equatable, Sendable {
  /// Whether this tool notifies at all.
  public var enabled: Bool
  /// Event kind (e.g. `blocked`) to on/off.
  public var events: [String: Bool]

  /// Settings with every field.
  public init(enabled: Bool, events: [String: Bool]) {
    self.enabled = enabled
    self.events = events
  }
}

/// One event a tool can notify about, as the server's catalog describes it.
public struct NotificationEventInfo: Codable, Equatable, Sendable, Identifiable {
  /// The event's kind, e.g. `blocked`.
  public let kind: String
  /// What the settings screen calls it.
  public let label: String
  /// Whether a new phone gets it on.
  public let `default`: Bool
  /// Whether it plays a sound.
  public let sound: Bool

  /// The kind, unique within a tool.
  public var id: String { kind }
}

/// A tool in the catalog: its name and the events it can notify about, in order.
public struct NotificationToolInfo: Codable, Equatable, Sendable, Identifiable {
  /// The tool, named like its `beagle` subcommand.
  public let tool: String
  /// Its display name.
  public let name: String
  /// Its events, in the order the settings screen lists them.
  public let events: [NotificationEventInfo]

  /// The tool, unique in the catalog.
  public var id: String { tool }
}

/// A phone's notification settings, plus the catalog of tools and events the screen is drawn from
/// (`GET /v1/devices/{device}/notifications`).
public struct NotificationDocument: Codable, Equatable, Sendable {
  /// Tool to its settings.
  public var settings: [String: ToolNotificationSettings]
  /// Every tool that can notify, in order.
  public let catalog: [NotificationToolInfo]

  /// A document with every field.
  public init(settings: [String: ToolNotificationSettings], catalog: [NotificationToolInfo]) {
    self.settings = settings
    self.catalog = catalog
  }

  /// A tool's settings, or the catalog's defaults when the server sent none.
  public func settings(for tool: NotificationToolInfo) -> ToolNotificationSettings {
    if let saved = settings[tool.tool] { return saved }
    let events = Dictionary(uniqueKeysWithValues: tool.events.map { ($0.kind, $0.default) })
    return ToolNotificationSettings(enabled: true, events: events)
  }
}

/// The answer to `POST /v1/devices/{device}/test`.
public struct TestPushResult: Codable, Equatable, Sendable {
  /// Whether APNs accepted the push.
  public let delivered: Bool
  /// What the Mac says happened.
  public let message: String
}

extension BeagleAPI {
  /// Device documents keep their keys as the server spells them: event kinds such as
  /// `project_started` are dictionary keys, which snake-case conversion would rename.
  public static let plainDecoder = JSONDecoder()

  private static func body(_ value: some Encodable) throws -> Data {
    let encoder = JSONEncoder()
    encoder.outputFormatting = .sortedKeys
    return try encoder.encode(value)
  }

  /// `PUT /v1/devices/{device}`: registers or refreshes this phone; answers its settings.
  public func registerDevice(_ device: String, _ registration: DeviceRegistration) async throws
    -> NotificationDocument
  {
    try await send(
      "PUT", ["v1", "devices", device], body: try Self.body(registration),
      decoder: Self.plainDecoder)
  }

  /// `DELETE /v1/devices/{device}`: the Mac forgets this phone.
  public func forgetDevice(_ device: String) async throws {
    try await send("DELETE", ["v1", "devices", device])
  }

  /// `GET /v1/devices/{device}/notifications`: this phone's settings and the catalog.
  public func notificationSettings(_ device: String) async throws -> NotificationDocument {
    try await send("GET", ["v1", "devices", device, "notifications"], decoder: Self.plainDecoder)
  }

  /// `PUT /v1/devices/{device}/notifications`: replaces this phone's settings; answers them as
  /// saved.
  public func saveNotificationSettings(
    _ device: String, _ settings: [String: ToolNotificationSettings]
  ) async throws -> NotificationDocument {
    try await send(
      "PUT", ["v1", "devices", device, "notifications"], body: try Self.body(settings),
      decoder: Self.plainDecoder)
  }

  /// `POST /v1/devices/{device}/test`: asks the Mac to push a test notification now.
  public func sendTestPush(_ device: String) async throws -> TestPushResult {
    try await send("POST", ["v1", "devices", device, "test"], decoder: Self.plainDecoder)
  }
}

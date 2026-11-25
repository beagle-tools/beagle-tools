import UIKit
import UserNotifications

/// Whether this app may show notifications.
public enum PushPermission: Sendable, Equatable {
  /// Never asked: the app may ask once.
  case notDetermined
  /// The user said no; only iOS Settings can change it.
  case denied
  /// Notifications are allowed.
  case authorized
}

/// The system calls push needs: the permission prompt and APNs registration.
///
/// Apps use ``SystemPushAuthorizer``; tests and the demo server use a fake.
@MainActor
public protocol PushAuthorizer: AnyObject {
  /// The current permission.
  func permission() async -> PushPermission
  /// Shows the system prompt (alert + sound); true when granted.
  func requestPermission() async throws -> Bool
  /// Asks APNs for a device token. The answer arrives through the app delegate, which hands it to
  /// ``Push/didRegister(deviceToken:)`` or ``Push/didFailToRegister(_:)``.
  func registerForRemoteNotifications()
}

/// The real thing: `UNUserNotificationCenter` and `UIApplication`.
public final class SystemPushAuthorizer: PushAuthorizer {
  /// The system authorizer.
  public init() {}

  /// The current permission, from the notification center.
  public func permission() async -> PushPermission {
    switch await UNUserNotificationCenter.current().notificationSettings().authorizationStatus {
    case .authorized, .provisional, .ephemeral: return .authorized
    case .denied: return .denied
    case .notDetermined: return .notDetermined
    @unknown default: return .notDetermined
    }
  }

  /// Asks for alerts and sounds (never badges: v1 has none).
  public func requestPermission() async throws -> Bool {
    try await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound])
  }

  /// Registers with APNs; iOS calls the app delegate back with the token.
  public func registerForRemoteNotifications() {
    UIApplication.shared.registerForRemoteNotifications()
  }
}

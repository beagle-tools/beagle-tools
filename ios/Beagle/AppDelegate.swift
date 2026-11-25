import BeaglePairing
import BeaglePush
import UIKit
import UserNotifications

/// What lives as long as the app: the pairing, push notifications, and links waiting to open.
///
/// It is the app delegate because APNs hands the device token to one, and the notification
/// center's delegate (taps, foreground banners) must be set before launch finishes.
final class AppDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
  let pairing: Pairing
  let push: Push
  let links = OpenedLinks()

  override init() {
    let setup = Self.setup()
    pairing = setup.pairing
    push = Push(
      pairing: setup.pairing, secrets: setup.secrets, authorizer: setup.authorizer,
      environment: Self.apnsEnvironment, topic: Bundle.main.bundleIdentifier ?? "",
      name: UIDevice.current.name)
    super.init()
    #if DEBUG
      (setup.authorizer as? DemoPushAuthorizer)?.push = push
    #endif
  }

  /// The APNs host this build's tokens belong to: Xcode (Debug) installs use the sandbox,
  /// TestFlight and App Store builds production.
  static var apnsEnvironment: String {
    #if DEBUG
      "development"
    #else
      "production"
    #endif
  }

  /// The Keychain service the token and install id are filed under.
  ///
  /// Keep it unchanged, so an existing pairing survives updates.
  static let keychainService = "dev.beagletools.beagle"

  /// The pairing, where secrets live, and who asks iOS for push.
  ///
  /// UI tests launch with `-ui-testing` (a throwaway Keychain service and defaults suite, so they
  /// never see or change a real pairing) or `-demo-server` (Debug only: paired with the in-app
  /// DemoServer, with a fake push permission and token).
  private static func setup() -> (
    pairing: Pairing, secrets: any SecretStore, authorizer: any PushAuthorizer
  ) {
    #if DEBUG
      if CommandLine.arguments.contains("-demo-server") {
        let pairing = Pairing(tool: "scout", preview: DemoServer.link, session: DemoServer.session)
        return (pairing, MemorySecretStore(), DemoPushAuthorizer())
      }
    #endif
    guard CommandLine.arguments.contains("-ui-testing") else {
      let keychain = Keychain(service: keychainService)
      return (Pairing(tool: "scout", secrets: keychain), keychain, SystemPushAuthorizer())
    }
    let suite = "dev.beagletools.beagle.uitests"
    let defaults = UserDefaults(suiteName: suite) ?? .standard
    defaults.removePersistentDomain(forName: suite)
    let keychain = Keychain(service: suite)
    try? keychain.delete(Pairing.tokenAccount)
    let pairing = Pairing(tool: "scout", secrets: keychain, defaults: defaults)
    return (pairing, keychain, SystemPushAuthorizer())
  }

  func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    UNUserNotificationCenter.current().delegate = self
    Task { await push.start() }
    return true
  }

  func application(
    _ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken token: Data
  ) {
    Task { await push.didRegister(deviceToken: token) }
  }

  func application(
    _ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: any Error
  ) {
    push.didFailToRegister(error)
  }

  /// Banners show while the app is open too.
  nonisolated func userNotificationCenter(
    _ center: UNUserNotificationCenter, willPresent notification: UNNotification
  ) async -> UNNotificationPresentationOptions {
    [.banner, .list, .sound]
  }

  /// A tap opens the notification's `beagle.link`, the same way a `beagle://` link does.
  nonisolated func userNotificationCenter(
    _ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse
  ) async {
    let beagle = response.notification.request.content.userInfo["beagle"] as? [String: Any]
    guard let link = beagle?["link"] as? String, let url = URL(string: link) else { return }
    await links.open(url)
  }
}

/// A `beagle://` link waiting for the navigation stack to open it (from a tapped notification).
@Observable
final class OpenedLinks {
  /// The link to open next.
  var pending: URL?

  /// Opens `url` as soon as the stack can.
  func open(_ url: URL) { pending = url }
}

#if DEBUG
  import BeaglePairing
  import BeaglePush
  import Foundation

  /// Push for the demo server: permission is granted when asked and the "APNs token" is fake, so
  /// UI tests reach the settings screen without the system prompt or Apple.
  final class DemoPushAuthorizer: PushAuthorizer {
    private var current = PushPermission.notDetermined
    /// The Push to hand the fake token to.
    weak var push: Push?

    func permission() async -> PushPermission { current }

    func requestPermission() async throws -> Bool {
      current = .authorized
      return true
    }

    func registerForRemoteNotifications() {
      Task { await push?.didRegister(deviceToken: Data(repeating: 0xab, count: 32)) }
    }

    /// A Push for previews, paired with the demo server.
    static func push(for pairing: Pairing) -> Push {
      let authorizer = DemoPushAuthorizer()
      let push = Push(
        pairing: pairing, secrets: MemorySecretStore(), authorizer: authorizer,
        environment: "development", topic: "dev.beagletools.beagle", name: "iPhone")
      authorizer.push = push
      return push
    }
  }

  /// The demo server's `/v1/devices` routes (docs/serve-api.md, Devices): phones and their
  /// settings in memory, with scout's catalog and defaults.
  nonisolated enum DemoDevices {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var settings: [String: Any] = [:]

    static var catalog: [[String: Any]] {
      [
        [
          "tool": "scout", "name": "Scout",
          "events": [
            event("blocked", "A task is blocked (needs you)", true, sound: true),
            event("project_started", "A project starts", true),
            event("project_done", "A project is done", true),
            event("task_done", "Every finished task", false),
            event("task_started", "Every task an agent picks up (noisy)", false),
            event("task_paused", "A working task went quiet", true),
          ],
        ]
      ]
    }

    static func event(_ kind: String, _ label: String, _ on: Bool, sound: Bool = false)
      -> [String: Any]
    {
      ["kind": kind, "label": label, "default": on, "sound": sound]
    }

    static var defaults: [String: Any] {
      let events = ["blocked", "project_started", "project_done", "task_paused"]
        .reduce(into: ["task_done": false, "task_started": false]) { $0[$1] = true }
      return ["scout": ["enabled": true, "events": events]]
    }

    /// Answers `method path` with `body`, or nil when it isn't a device route.
    static func answer(_ method: String, _ path: String, _ body: Data) -> (Int, Any)? {
      let parts = path.split(separator: "/").map(String.init)
      guard parts.count >= 3, parts[0] == "v1", parts[1] == "devices" else { return nil }
      let device = parts[2]
      return lock.withLock {
        let known = settings[device] != nil
        switch (method, parts.dropFirst(3).first) {
        case ("PUT", nil):
          if !known { settings[device] = defaults }
          return (known ? 200 : 201, document(device))
        case ("DELETE", nil):
          settings[device] = nil
          return (204, [String: Any]())
        case (_, _) where !known:
          return (404, DemoData.error("not_found", "no device \(device)"))
        case ("GET", "notifications"):
          return (200, document(device))
        case ("PUT", "notifications"):
          guard let saved = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
            return (400, DemoData.error("bad_request", "settings must be a JSON object"))
          }
          settings[device] = saved
          return (200, document(device))
        case ("POST", "test"):
          return (200, ["delivered": true, "message": "Sent a test push to iPhone (demo)."])
        default:
          return (404, DemoData.error("not_found", "no route \(path)"))
        }
      }
    }

    private static func document(_ device: String) -> [String: Any] {
      ["settings": settings[device] ?? defaults, "catalog": catalog]
    }
  }

  extension URLRequest {
    /// The body, which URLProtocol sees as a stream.
    nonisolated var bodyData: Data {
      if let httpBody { return httpBody }
      guard let stream = httpBodyStream else { return Data() }
      stream.open()
      defer { stream.close() }
      var data = Data()
      var buffer = [UInt8](repeating: 0, count: 4096)
      while case let read = stream.read(&buffer, maxLength: buffer.count), read > 0 {
        data.append(buffer, count: read)
      }
      return data
    }
  }
#endif

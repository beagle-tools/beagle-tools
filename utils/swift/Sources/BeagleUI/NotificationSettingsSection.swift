import BeagleAPI
import BeaglePush
import SwiftUI
import UIKit

/// The Notifications part of an app's Settings.
///
/// Permission, registration with the Mac, then one section per tool drawn from the server's catalog (a master switch and one switch per event),
/// and a test button. Changes save to the Mac right away.
///
/// Reads the `Push` from the environment. Use it inside a `Form`.
public struct NotificationSettingsSection: View {
  @Environment(Push.self) private var push
  @State private var testResult: String?
  @State private var testing = false

  /// The sections.
  public init() {}

  /// The view's content.
  public var body: some View {
    Section {
      permissionRows
    } header: {
      Text("Notifications")
    } footer: {
      Text(
        "Your Mac sends these through Apple while `beagle serve` runs, also when the app is closed."
      )
    }
    // Shows the Mac's current settings each time the screen opens.
    .task { await push.refresh() }
    if case .registered(let document) = push.state {
      ForEach(document.catalog) { tool in
        ToolNotificationsSection(tool: tool, settings: document.settings(for: tool))
      }
      Section {
        Button(testing ? "Sending…" : "Send a test notification") { Task { await test() } }
          .disabled(testing)
          .accessibilityIdentifier("notifications.test")
        if let testResult {
          Text(testResult).font(.footnote).accessibilityIdentifier("notifications.testResult")
        }
      }
    }
    if let problem = push.problem {
      Text(problem).foregroundStyle(.red).accessibilityIdentifier("notifications.problem")
    }
  }

  @ViewBuilder private var permissionRows: some View {
    switch push.permission {
    case .notDetermined:
      Button("Turn on notifications") { Task { await push.requestPermission() } }
        .accessibilityIdentifier("notifications.turnOn")
    case .denied:
      Text("Notifications are off for Beagle in iOS Settings.").foregroundStyle(.secondary)
      Button("Open iOS Settings") {
        if let url = URL(string: UIApplication.openNotificationSettingsURLString) {
          UIApplication.shared.open(url)
        }
      }
      .accessibilityIdentifier("notifications.openSettings")
    case .authorized:
      registrationRow
    }
  }

  @ViewBuilder private var registrationRow: some View {
    switch push.state {
    case .idle:
      Text("Pair with your Mac to get notifications.").foregroundStyle(.secondary)
    case .waitingForToken, .registering:
      LabeledContent("Status") { ProgressView() }
    case .registered:
      LabeledContent("Status", value: "On")
        .accessibilityIdentifier("notifications.registered")
    case .failed(let message):
      LabeledContent("Status") {
        Text(message).foregroundStyle(.red).multilineTextAlignment(.trailing)
      }
      Button("Try again") { Task { await push.sync() } }
    }
  }

  private func test() async {
    testing = true
    testResult = await push.sendTest()
    testing = false
  }
}

/// One tool's switches, labelled as the catalog says.
struct ToolNotificationsSection: View {
  @Environment(Push.self) private var push
  let tool: NotificationToolInfo
  let settings: ToolNotificationSettings

  var body: some View {
    Section(tool.name) {
      Toggle(
        "Notify me",
        isOn: binding(settings.enabled) { on in
          await push.setEnabled(on, for: tool)
        }
      )
      .accessibilityIdentifier("notifications.\(tool.tool).enabled")
      ForEach(tool.events) { event in
        Toggle(
          isOn: binding(settings.events[event.kind] ?? event.default) { on in
            await push.setEvent(event.kind, on, for: tool)
          }
        ) {
          Text(event.label)
          if event.sound { Text("Plays a sound") }
        }
        .disabled(!settings.enabled)
        .accessibilityIdentifier("notifications.\(tool.tool).\(event.kind)")
      }
    }
  }

  private func binding(
    _ value: Bool, save: @escaping @MainActor (Bool) async -> Void
  ) -> Binding<Bool> {
    Binding(get: { value }, set: { on in Task { await save(on) } })
  }
}

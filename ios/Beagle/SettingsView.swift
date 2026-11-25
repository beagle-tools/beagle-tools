import BeagleUI
import SwiftUI

/// The Settings screen: the paired Mac (BeagleUI's ServerSection), Notifications, and the app
/// version.
struct SettingsView: View {
  var body: some View {
    Form {
      ServerSection()
      Section {
        NavigationLink(value: HomeRoute.notifications) {
          Label("Notifications", systemImage: "bell.badge")
        }
        .accessibilityIdentifier("settings.notifications")
      }
      Section("About") {
        LabeledContent("Version", value: AppInfo.version)
      }
    }
    .navigationTitle("Settings")
  }
}

/// Settings → Notifications: permission, then which events each tool notifies about
/// (BeagleUI's NotificationSettingsSection, drawn from the Mac's catalog).
struct NotificationsView: View {
  var body: some View {
    Form {
      NotificationSettingsSection()
    }
    .navigationTitle("Notifications")
  }
}

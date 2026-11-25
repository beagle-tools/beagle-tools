import BeagleUI
import SwiftUI

/// The Settings tab: the paired Mac (BeagleUI's ServerSection) and the app version.
struct SettingsView: View {
  var body: some View {
    Form {
      ServerSection()
      Section("About") {
        LabeledContent("Version", value: AppInfo.version)
      }
    }
    .navigationTitle("Settings")
  }
}

import SwiftUI

/// The Settings tab: which Mac this phone is paired with, and whether it answers.
struct SettingsView: View {
  @Environment(Pairing.self) private var pairing
  @State private var showPair = false
  @State private var status: Status = .unknown
  @State private var problem: String?

  enum Status: Equatable {
    case unknown, checking
    case connected(ServerVersion)
    case failed(String)
  }

  var body: some View {
    Form {
      Section("Server") {
        if let current = pairing.current {
          LabeledContent("Mac", value: current.displayName)
            .accessibilityIdentifier("settings.server")
          statusRow
          Button("Pair again") { showPair = true }
          Button("Unpair", role: .destructive) { unpair() }
        } else {
          Text("Not paired")
            .foregroundStyle(.secondary)
            .accessibilityIdentifier("settings.notPaired")
          Button("Pair with your Mac") { showPair = true }
            .accessibilityIdentifier("settings.pair")
        }
        if let problem {
          Text(problem).foregroundStyle(.red)
        }
      }
      Section("About") {
        LabeledContent("Version", value: AppInfo.version)
      }
    }
    .navigationTitle("Settings")
    .sheet(isPresented: $showPair) { PairView() }
    .task(id: pairing.current?.server) { await check() }
    .refreshable { await check() }
  }

  @ViewBuilder private var statusRow: some View {
    switch status {
    case .unknown, .checking:
      LabeledContent("Status") { ProgressView() }
    case .connected(let version):
      LabeledContent("Status", value: "Connected · beagle \(version.beagle)")
        .accessibilityIdentifier("settings.connected")
    case .failed(let message):
      LabeledContent("Status") {
        Text(message).foregroundStyle(.red).multilineTextAlignment(.trailing)
      }
    }
  }

  private func check() async {
    guard let api = pairing.api else {
      status = .unknown
      return
    }
    status = .checking
    do {
      status = .connected(try await api.version())
    } catch {
      status = .failed(error.localizedDescription)
    }
  }

  private func unpair() {
    do {
      try pairing.unpair()
      problem = nil
    } catch {
      problem = error.localizedDescription
    }
  }
}

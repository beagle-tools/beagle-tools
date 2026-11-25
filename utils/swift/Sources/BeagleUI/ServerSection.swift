import BeagleAPI
import BeaglePairing
import SwiftUI

/// The "Server" section of an app's Settings: which Mac it's paired with,
/// whether it answers, and Pair / Pair again / Unpair. Reads the ``Pairing``
/// from the environment.
public struct ServerSection: View {
  @Environment(Pairing.self) private var pairing
  @State private var showPair = false
  @State private var status: Status = .unknown
  @State private var problem: String?

  enum Status: Equatable {
    case unknown, checking
    case connected(ServerVersion)
    case failed(String)
  }

  /// The section; checks the connection whenever the pairing changes.
  public init() {}

  public var body: some View {
    Section("Server") {
      if let current = pairing.current {
        LabeledContent("Mac", value: current.displayName)
          .accessibilityIdentifier("settings.server")
        statusRow
        Button("Check again") { Task { await check() } }
        // The sheet hangs off a row: a .sheet on a Section inside a Form never shows.
        Button("Pair again") { showPair = true }
          .sheet(isPresented: $showPair) { PairView() }
        Button("Unpair", role: .destructive) { unpair() }
      } else {
        Text("Not paired")
          .foregroundStyle(.secondary)
          .accessibilityIdentifier("settings.notPaired")
        Button("Pair with your Mac") { showPair = true }
          .accessibilityIdentifier("settings.pair")
          .sheet(isPresented: $showPair) { PairView() }
      }
      if let problem {
        Text(problem).foregroundStyle(.red)
      }
    }
    .task(id: pairing.current?.server) { await check() }
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

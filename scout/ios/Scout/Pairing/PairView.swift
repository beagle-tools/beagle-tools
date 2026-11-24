import SwiftUI

/// Pairs the app with `beagle serve`: scan the QR code `beagle serve pair`
/// prints, or paste its link, or type the server and token.
struct PairView: View {
  @Environment(Pairing.self) private var pairing
  @Environment(\.dismiss) private var dismiss

  /// A link that opened the app (`beagle://pair?…`), shown for confirmation.
  var initialLink: String = ""

  @State private var link = ""
  @State private var server = ""
  @State private var token = ""
  @State private var scanning = false
  @State private var working = false
  @State private var problem: String?

  var body: some View {
    NavigationStack {
      Form {
        Section {
          if QRScanner.isAvailable {
            Button("Scan the QR code", systemImage: "qrcode.viewfinder") { scanning = true }
          } else {
            Label("Scanning needs the camera. Paste the link instead.", systemImage: "camera")
              .foregroundStyle(.secondary)
          }
        } footer: {
          Text("On your Mac, run `beagle serve pair` and scan the code it shows.")
        }

        Section("Or paste the link") {
          TextField("beagle://pair?url=…&token=…", text: $link, axis: .vertical)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .accessibilityIdentifier("pair.link")
        }

        Section("Or type the server and token") {
          TextField("https://mac.example.ts.net:8787", text: $server)
            .keyboardType(.URL)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .accessibilityIdentifier("pair.server")
          SecureField("Token", text: $token)
            .accessibilityIdentifier("pair.token")
        }

        if let problem {
          Section {
            Label(problem, systemImage: "exclamationmark.triangle")
              .foregroundStyle(.red)
              .accessibilityIdentifier("pair.problem")
          }
        }
      }
      .navigationTitle("Pair with your Mac")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
        ToolbarItem(placement: .confirmationAction) {
          if working {
            ProgressView()
          } else {
            Button("Pair") { Task { await pair() } }
              .disabled(link.isEmpty && (server.isEmpty || token.isEmpty))
              .accessibilityIdentifier("pair.submit")
          }
        }
      }
      .fullScreenCover(isPresented: $scanning) {
        ScannerSheet { scanned in
          scanning = false
          link = scanned
          Task { await pair() }
        }
      }
      .onAppear { if link.isEmpty { link = initialLink } }
    }
  }

  private func pair() async {
    problem = nil
    working = true
    defer { working = false }
    do {
      let parsed =
        link.isEmpty
        ? try PairingLink(server: server, token: token) : try PairingLink.parse(link)
      try await pairing.pair(parsed)
      dismiss()
    } catch {
      problem = error.localizedDescription
    }
  }
}

/// The camera with a close button over it.
private struct ScannerSheet: View {
  let onScan: (String) -> Void
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    QRScanner(onScan: onScan)
      .ignoresSafeArea()
      .overlay(alignment: .topTrailing) {
        Button("Close", systemImage: "xmark.circle.fill") { dismiss() }
          .labelStyle(.iconOnly)
          .font(.largeTitle)
          .padding()
      }
  }
}

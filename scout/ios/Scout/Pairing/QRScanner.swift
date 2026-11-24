import SwiftUI
import VisionKit

/// A full-screen camera that reports the first QR code it reads.
/// Uses VisionKit's DataScannerViewController (needs a camera; not in the simulator).
struct QRScanner: UIViewControllerRepresentable {
  let onScan: (String) -> Void

  /// True when this device can scan (a camera, and camera access not refused).
  static var isAvailable: Bool {
    DataScannerViewController.isSupported && DataScannerViewController.isAvailable
  }

  func makeUIViewController(context: Context) -> DataScannerViewController {
    let scanner = DataScannerViewController(
      recognizedDataTypes: [.barcode(symbologies: [.qr])],
      qualityLevel: .balanced,
      isHighlightingEnabled: true)
    scanner.delegate = context.coordinator
    return scanner
  }

  func updateUIViewController(_ scanner: DataScannerViewController, context: Context) {
    if !scanner.isScanning {
      try? scanner.startScanning()
    }
  }

  static func dismantleUIViewController(
    _ scanner: DataScannerViewController, coordinator: Coordinator
  ) {
    scanner.stopScanning()
  }

  func makeCoordinator() -> Coordinator { Coordinator(onScan: onScan) }

  final class Coordinator: NSObject, DataScannerViewControllerDelegate {
    let onScan: (String) -> Void
    private var done = false

    init(onScan: @escaping (String) -> Void) { self.onScan = onScan }

    func dataScanner(
      _ dataScanner: DataScannerViewController, didAdd addedItems: [RecognizedItem],
      allItems: [RecognizedItem]
    ) {
      guard !done else { return }
      for item in addedItems {
        if case .barcode(let code) = item, let text = code.payloadStringValue {
          done = true
          dataScanner.stopScanning()
          onScan(text)
          return
        }
      }
    }
  }
}

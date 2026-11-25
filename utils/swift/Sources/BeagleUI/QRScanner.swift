import SwiftUI
import VisionKit

/// A full-screen camera that reports the first QR code it reads.
/// Uses VisionKit's DataScannerViewController (needs a camera; not in the simulator).
public struct QRScanner: UIViewControllerRepresentable {
  let onScan: (String) -> Void

  /// Calls `onScan` once, with the first QR code's text.
  public init(onScan: @escaping (String) -> Void) { self.onScan = onScan }

  /// True when this device can scan (a camera, and camera access not refused).
  public static var isAvailable: Bool {
    DataScannerViewController.isSupported && DataScannerViewController.isAvailable
  }

  public func makeUIViewController(context: Context) -> DataScannerViewController {
    let scanner = DataScannerViewController(
      recognizedDataTypes: [.barcode(symbologies: [.qr])],
      qualityLevel: .balanced,
      isHighlightingEnabled: true)
    scanner.delegate = context.coordinator
    return scanner
  }

  public func updateUIViewController(_ scanner: DataScannerViewController, context: Context) {
    if !scanner.isScanning {
      try? scanner.startScanning()
    }
  }

  public static func dismantleUIViewController(
    _ scanner: DataScannerViewController, coordinator: Coordinator
  ) {
    scanner.stopScanning()
  }

  public func makeCoordinator() -> Coordinator { Coordinator(onScan: onScan) }

  /// Receives the scanner's results.
  public final class Coordinator: NSObject, DataScannerViewControllerDelegate {
    let onScan: (String) -> Void
    private var done = false

    init(onScan: @escaping (String) -> Void) { self.onScan = onScan }

    public func dataScanner(
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

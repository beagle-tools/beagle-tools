import Foundation
import Testing

/// JSON captured from a real `beagle serve --no-tls` on loopback (Fixtures/).
func fixture(_ name: String) -> Data {
  guard let url = Bundle.module.url(forResource: "Fixtures/\(name)", withExtension: "json"),
    let data = try? Data(contentsOf: url)
  else {
    Issue.record("missing fixture \(name).json")
    return Data()
  }
  return data
}

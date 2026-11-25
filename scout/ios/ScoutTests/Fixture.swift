import Foundation

/// JSON captured from a real `beagle serve --no-tls` on loopback, reading a
/// neutral example scout store (ScoutTests/Fixtures).
enum Fixture {
  private final class Token {}

  static func data(_ name: String) -> Data {
    let bundle = Bundle(for: Token.self)
    guard let url = bundle.url(forResource: name, withExtension: "json"),
      let data = try? Data(contentsOf: url)
    else { preconditionFailure("missing fixture \(name).json") }
    return data
  }
}

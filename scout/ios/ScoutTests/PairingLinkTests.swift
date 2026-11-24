import Foundation
import Testing

@testable import Scout

struct PairingLinkTests {
  let token = Fixture.token

  @Test func parsesTheLinkBeagleServePairPrints() throws {
    let link = try PairingLink.parse(
      "beagle://pair?url=https%3A%2F%2Fmac.example.ts.net%3A8787&token=\(token)")
    #expect(link.server.absoluteString == "https://mac.example.ts.net:8787")
    #expect(link.token == token)
    #expect(link.displayName == "mac.example.ts.net:8787")
  }

  @Test func toleratesWhitespaceCaseAndParameterOrder() throws {
    let link = try PairingLink.parse(
      "  BEAGLE://PAIR?token=\(token.uppercased())&url=https%3A%2F%2Fmac.example.ts.net%3A8787\n")
    #expect(link.token == token)
    #expect(link.server.host() == "mac.example.ts.net")
  }

  @Test func acceptsAnUnencodedURLValue() throws {
    let link = try PairingLink.parse(
      "beagle://pair?url=https://mac.example.ts.net:8787/&token=\(token)")
    #expect(link.server.absoluteString == "https://mac.example.ts.net:8787")
  }

  @Test(arguments: [
    ("https://example.com", PairingLink.Problem.notAPairingLink),
    ("beagle://other?url=x&token=y", .notAPairingLink),
    ("not a link", .notAPairingLink),
    ("beagle://pair?token=\(Fixture.token)", .missingServer),
    ("beagle://pair?url=https%3A%2F%2Fmac.example.ts.net%3A8787", .missingToken),
    ("beagle://pair?url=https%3A%2F%2Fmac.example.ts.net%3A8787&token=abc", .badToken),
    (
      "beagle://pair?url=http%3A%2F%2Fmac.example.ts.net%3A8787&token=\(Fixture.token)",
      .insecureServer
    ),
  ])
  func rejectsBadLinks(text: String, problem: PairingLink.Problem) {
    #expect(throws: problem) { try PairingLink.parse(text) }
  }

  @Test func manualEntryValidatesTheServer() throws {
    #expect(throws: PairingLink.Problem.badServer("https://mac.example.ts.net:8787/v1")) {
      try PairingLink(server: "https://mac.example.ts.net:8787/v1", token: token)
    }
    #expect(throws: PairingLink.Problem.badServer("mac.example.ts.net")) {
      try PairingLink(server: "mac.example.ts.net", token: token)
    }
    let typed = try PairingLink(server: " https://mac.example.ts.net:8787/ ", token: " \(token) ")
    #expect(typed.server.absoluteString == "https://mac.example.ts.net:8787")
  }

  @Test func plainHTTPOnlyOnLoopback() throws {
    let local = try PairingLink(server: "http://127.0.0.1:8787", token: token)
    #expect(local.server.scheme == "http")
    #expect(throws: PairingLink.Problem.insecureServer) {
      try PairingLink(server: "http://192.168.1.20:8787", token: token)
    }
  }

  @Test func problemsReadPlainly() {
    #expect(PairingLink.Problem.notAPairingLink.localizedDescription.contains("beagle serve pair"))
  }
}

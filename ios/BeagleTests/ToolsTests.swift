import Testing

@testable import Beagle

/// The home screen's list of tools.
@MainActor
struct ToolsTests {
  @Test func scoutComesFirstAndHasScreens() throws {
    let first = try #require(Tools.all.first)
    #expect(first.id == "scout")
    #expect(first.isAvailable)
  }

  @Test func listsEveryBeagleToolOnce() {
    // The `beagle` subcommands that are tools (beagle/src/tools.rs, without serve).
    let ids = Tools.all.map(\.id)
    #expect(Set(ids) == ["scout", "recap", "standup", "rca", "tandem", "agent"])
    #expect(Set(ids).count == ids.count)
  }

  @Test func toolsWithoutScreensAreComingSoon() {
    for tool in Tools.all where tool.id != "scout" {
      #expect(!tool.isAvailable, "\(tool.id)")
    }
  }

  @Test func findsAToolByName() {
    #expect(Tools.named("scout")?.name == "Scout")
    #expect(Tools.named("serve") == nil)
  }
}

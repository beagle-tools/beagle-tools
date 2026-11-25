import Foundation
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

/// Where a tapped notification's `beagle://` link goes (docs/serve-push.md).
@MainActor
struct LinkTests {
  @Test func aTaskLinkOpensTheProjectThenTheTask() throws {
    let routes = try #require(
      ScoutTool.links(["projects", "rate-limit-the-api", "tasks", "03-pr"]))
    #expect(routes.count == 2)
    #expect(
      routes[0] as? ProjectRoute
        == ProjectRoute(id: "rate-limit-the-api", title: "rate-limit-the-api"))
    #expect(
      routes[1] as? TaskRoute
        == TaskRoute(project: "rate-limit-the-api", task: "03-pr", title: "03-pr"))
  }

  @Test func aProjectLinkStopsAtTheProject() throws {
    let routes = try #require(ScoutTool.links(["projects", "tidy-the-docs"]))
    #expect(
      routes.map { $0 as? ProjectRoute } == [
        ProjectRoute(id: "tidy-the-docs", title: "tidy-the-docs")
      ])
    #expect(ScoutTool.links([])?.isEmpty == true)
  }

  @Test func unknownShapesOpenNothing() {
    #expect(ScoutTool.links(["projects", "a", "log"]) == nil)
    #expect(ScoutTool.links(["rca", "x"]) == nil)
  }

  @Test func theToolRegistryRoutesByHost() throws {
    let link = try #require(URL(string: "beagle://scout/projects/p/tasks/01-t"))
    #expect(Tools.path(for: link)?.count == 3)
    #expect(Tools.path(for: try #require(URL(string: "beagle://scout")))?.count == 1)
    #expect(Tools.path(for: try #require(URL(string: "beagle://pair?server=x&token=y"))) == nil)
    #expect(Tools.path(for: try #require(URL(string: "beagle://recap/x"))) == nil)
    #expect(Tools.path(for: try #require(URL(string: "https://scout/projects/p"))) == nil)
  }
}

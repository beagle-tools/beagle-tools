import Testing

@testable import Scout

@MainActor
struct AppInfoTests {
  @Test func versionShowsMarketingVersionAndBuild() {
    // The test bundle is hosted by the app, so Bundle.main is Scout.app.
    #expect(AppInfo.version.contains("("))
    #expect(!AppInfo.version.hasPrefix("?"))
  }
}

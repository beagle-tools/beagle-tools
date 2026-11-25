import Testing

@testable import Beagle

@MainActor
struct AppInfoTests {
  @Test func versionShowsMarketingVersionAndBuild() {
    // The test bundle is hosted by the app, so Bundle.main is Beagle.app.
    #expect(AppInfo.version.contains("("))
    #expect(!AppInfo.version.hasPrefix("?"))
  }
}

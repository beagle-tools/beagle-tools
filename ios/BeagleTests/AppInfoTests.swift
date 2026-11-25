import Foundation
import Testing

@testable import Beagle

@MainActor
struct AppInfoTests {
  @Test func versionShowsMarketingVersionAndBuild() {
    // The test bundle is hosted by the app, so Bundle.main is Beagle.app.
    #expect(AppInfo.version.contains("("))
    #expect(!AppInfo.version.hasPrefix("?"))
  }

  @Test func appHasTheBeagleIcon() throws {
    // The asset catalog's AppIcon ends up as the bundle's primary icon.
    let icons = try #require(Bundle.main.infoDictionary?["CFBundleIcons"] as? [String: Any])
    let primary = try #require(icons["CFBundlePrimaryIcon"] as? [String: Any])
    #expect(primary["CFBundleIconName"] as? String == "AppIcon")
  }
}

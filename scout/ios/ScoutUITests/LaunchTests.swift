import XCTest

/// Smoke test: the app launches and both tabs are reachable.
final class LaunchTests: XCTestCase {
  override func setUp() {
    continueAfterFailure = false
  }

  @MainActor
  func testLaunchShowsScoutAndSettingsTabs() {
    let app = XCUIApplication()
    app.launch()

    // Tab buttons are matched anywhere: newer iOS versions don't always put
    // them under `tabBars`.
    XCTAssertTrue(app.navigationBars["Scout"].waitForExistence(timeout: 10))
    app.buttons["Settings"].firstMatch.tap()
    XCTAssertTrue(app.navigationBars["Settings"].waitForExistence(timeout: 5))
  }
}

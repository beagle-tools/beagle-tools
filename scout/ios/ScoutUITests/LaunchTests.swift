import XCTest

/// Smoke tests: the app launches unpaired (`-ui-testing` gives it a throwaway
/// Keychain and defaults), and the pairing screen opens and validates input.
final class LaunchTests: XCTestCase {
  override func setUp() {
    continueAfterFailure = false
  }

  @MainActor
  private func launch() -> XCUIApplication {
    let app = XCUIApplication()
    app.launchArguments = ["-ui-testing"]
    app.launch()
    return app
  }

  @MainActor
  func testLaunchesUnpairedAndSettingsOffersPairing() {
    let app = launch()

    // Tab buttons are matched anywhere: newer iOS versions don't always put
    // them under `tabBars`.
    XCTAssertTrue(app.navigationBars["Scout"].waitForExistence(timeout: 10))
    XCTAssertTrue(app.staticTexts["Not paired"].exists)
    app.buttons["Settings"].firstMatch.tap()
    XCTAssertTrue(app.navigationBars["Settings"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.staticTexts["settings.notPaired"].exists)
  }

  @MainActor
  func testPairScreenRejectsABadLink() {
    let app = launch()
    app.buttons["Settings"].firstMatch.tap()
    app.buttons["settings.pair"].tap()

    let field = app.textFields["pair.link"]
    XCTAssertTrue(field.waitForExistence(timeout: 5))
    field.tap()
    field.typeText("https://example.com")
    app.buttons["pair.submit"].tap()
    XCTAssertTrue(app.staticTexts["pair.problem"].waitForExistence(timeout: 5))
  }

  /// End to end against a real `beagle serve --no-tls` on this Mac's loopback.
  /// Runs only when the server is given, e.g.:
  ///   TEST_RUNNER_SCOUT_E2E_SERVER=http://127.0.0.1:18787 TEST_RUNNER_SCOUT_E2E_TOKEN=<hex> \
  ///     xcodebuild test …
  @MainActor
  func testPairsWithALocalServer() throws {
    let env = ProcessInfo.processInfo.environment
    guard let server = env["SCOUT_E2E_SERVER"], let token = env["SCOUT_E2E_TOKEN"] else {
      throw XCTSkip("set SCOUT_E2E_SERVER and SCOUT_E2E_TOKEN to run against a local beagle serve")
    }
    let app = launch()
    app.buttons["Settings"].firstMatch.tap()
    app.buttons["settings.pair"].tap()

    let serverField = app.textFields["pair.server"]
    XCTAssertTrue(serverField.waitForExistence(timeout: 5))
    serverField.tap()
    serverField.typeText(server)
    let tokenField = app.secureTextFields["pair.token"]
    tokenField.tap()
    tokenField.typeText(token)
    app.buttons["pair.submit"].tap()

    XCTAssertTrue(app.descendants(matching: .any)["settings.server"].waitForExistence(timeout: 10))
    let connected = app.descendants(matching: .any)["settings.connected"]
    XCTAssertTrue(connected.waitForExistence(timeout: 10))
  }
}

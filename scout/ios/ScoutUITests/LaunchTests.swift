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

    // Optionally: a project and task the local store holds, as `beagle scout show` lists them.
    guard let project = env["SCOUT_E2E_PROJECT"], let task = env["SCOUT_E2E_TASK"] else { return }
    // iOS may still offer to save the typed token as a password.
    if app.buttons["Not Now"].waitForExistence(timeout: 2) { app.buttons["Not Now"].tap() }
    let row = app.buttons["project.\(project)"]
    for _ in 0..<3 where !row.exists {
      app.buttons["Scout"].firstMatch.tap()
      _ = row.waitForExistence(timeout: 5)
    }
    XCTAssertTrue(row.exists)
    let taskRow = app.buttons["task.\(task)"]
    // A tap can land while a refresh redraws the list; tap again if so.
    for _ in 0..<3 where !taskRow.exists {
      if row.exists { row.tap() }
      _ = taskRow.waitForExistence(timeout: 5)
    }
    XCTAssertTrue(taskRow.exists)
    if let progress = env["SCOUT_E2E_TASK_PROGRESS"] {
      XCTAssertTrue(app.buttons["task.\(task)"].label.contains(progress))
    }
  }
}

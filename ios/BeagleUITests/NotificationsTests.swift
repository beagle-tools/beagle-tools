import XCTest

/// Settings → Notifications and notification links, against the in-app demo server
/// (`-demo-server`: permission is granted when asked and the APNs token is fake).
final class NotificationsTests: XCTestCase {
  override func setUp() {
    continueAfterFailure = false
  }

  @MainActor
  private func launch() -> XCUIApplication {
    let app = XCUIApplication()
    app.launchArguments = ["-demo-server"]
    app.launch()
    return app
  }

  /// Scout's events with scout.toml's defaults (`NotifyEvents::default()`).
  private let defaults = [
    "blocked": "1", "project_started": "1", "project_done": "1",
    "task_done": "0", "task_started": "0", "task_paused": "1",
  ]

  @MainActor
  private func value(_ app: XCUIApplication, _ kind: String) -> String? {
    app.switches["notifications.scout.\(kind)"].value as? String
  }

  @MainActor
  func testShowsScoutsEventsWithItsDefaultsAndSavesChangesToTheMac() {
    let app = launch()
    XCTAssertTrue(app.buttons["home.settings"].waitForExistence(timeout: 10))
    app.buttons["home.settings"].tap()
    app.buttons["settings.notifications"].tap()

    // Never asked at launch: the screen offers to turn them on.
    let turnOn = app.buttons["notifications.turnOn"]
    XCTAssertTrue(turnOn.waitForExistence(timeout: 5))
    turnOn.tap()

    // Registered with the Mac: one section for Scout, its six events, scout's defaults.
    XCTAssertTrue(app.switches["notifications.scout.blocked"].waitForExistence(timeout: 10))
    XCTAssertTrue(app.staticTexts["notifications.registered"].exists)
    XCTAssertTrue(app.switches["notifications.scout.enabled"].exists)
    for (kind, on) in defaults {
      XCTAssertEqual(value(app, kind), on, kind)
    }
    let shot = XCTAttachment(screenshot: app.screenshot())
    shot.name = "notifications"
    shot.lifetime = .keepAlways
    add(shot)

    // Turning on "Every finished task" saves it to the Mac…
    app.switches["notifications.scout.task_done"].switches.firstMatch.tap()
    let saved = NSPredicate(format: "value == '1'")
    expectation(for: saved, evaluatedWith: app.switches["notifications.scout.task_done"])
    waitForExpectations(timeout: 5)

    // …so leaving and coming back (the screen reloads from the Mac) still shows it on.
    app.navigationBars["Notifications"].buttons.firstMatch.tap()
    app.buttons["settings.notifications"].tap()
    XCTAssertTrue(app.switches["notifications.scout.task_done"].waitForExistence(timeout: 5))
    XCTAssertEqual(value(app, "task_done"), "1")
    XCTAssertEqual(value(app, "task_started"), "0")

    // The test button asks the Mac to push one now.
    let test = app.buttons["notifications.test"]
    if !test.isHittable { app.swipeUp() }
    test.tap()
    XCTAssertTrue(app.staticTexts["notifications.testResult"].waitForExistence(timeout: 5))
  }

  @MainActor
  func testANotificationLinkOpensTheTask() throws {
    let app = launch()
    XCTAssertTrue(app.navigationBars["Beagle"].waitForExistence(timeout: 10))
    let link = try XCTUnwrap(
      URL(string: "beagle://scout/projects/rate-limit-the-api/tasks/02-add-allowlist"))
    app.open(link)

    // home → Scout → the project → the task, titled once it loads.
    XCTAssertTrue(app.navigationBars["02 · Add the allowlist"].waitForExistence(timeout: 10))
    app.navigationBars.buttons.firstMatch.tap()
    XCTAssertTrue(app.navigationBars["Rate-limit the public API"].waitForExistence(timeout: 5))
  }
}

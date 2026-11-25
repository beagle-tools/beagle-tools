import XCTest

/// The home screen, then the Scout screens against the in-app demo server
/// (`-demo-server`, Debug builds): home → projects → project tabs → task tabs.
final class ScoutScreensTests: XCTestCase {
  override func setUp() {
    continueAfterFailure = false
  }

  @MainActor
  func testBrowseProjectTasksAndLog() {
    let app = XCUIApplication()
    app.launchArguments = ["-demo-server"]
    app.launch()

    // Home: every tool, Scout available, the rest coming soon.
    XCTAssertTrue(app.navigationBars["Beagle"].waitForExistence(timeout: 10))
    XCTAssertFalse(app.buttons["home.notPaired"].exists)
    for tool in ["recap", "standup", "rca", "tandem", "agent"] {
      XCTAssertTrue(app.descendants(matching: .any)["tool.\(tool)"].exists, tool)
    }
    snapshot(app, "home")
    app.buttons["tool.scout"].tap()

    // Projects: open and closed sections, the working project first.
    let project = app.buttons["project.rate-limit-the-api"]
    XCTAssertTrue(project.waitForExistence(timeout: 10))
    XCTAssertTrue(app.staticTexts["Rate-limit the public API"].exists)
    XCTAssertTrue(app.staticTexts["Tidy the docs"].exists)
    project.tap()

    // Project: handoff and tasks.
    XCTAssertTrue(app.buttons["task.02-add-allowlist"].waitForExistence(timeout: 10))
    XCTAssertTrue(app.staticTexts["Next steps · from 01-map-request-path"].exists)
    snapshot(app, "project")

    // Log tab, newest first.
    app.segmentedControls["project.tabs"].buttons["Log"].tap()
    XCTAssertTrue(
      app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'add the allowlist table'"))
        .firstMatch.waitForExistence(timeout: 10))
    snapshot(app, "log")

    // Task: plan sections, then the outcome tab.
    app.segmentedControls["project.tabs"].buttons["Tasks"].tap()
    app.buttons["task.01-map-request-path"].tap()
    XCTAssertTrue(app.staticTexts["task.status"].waitForExistence(timeout: 10))
    XCTAssertEqual(app.staticTexts["task.status"].label, "Done")
    XCTAssertTrue(app.staticTexts["CONTEXT"].exists || app.staticTexts["Context"].exists)
    snapshot(app, "task")
    app.segmentedControls["task.tabs"].buttons["Outcome"].tap()
    XCTAssertTrue(
      app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'middleware/limit.rs'"))
        .firstMatch.waitForExistence(timeout: 5))
    snapshot(app, "outcome")
  }

  /// A task waiting on a merge shows as waiting (◷), not unknown, in the list and on the task.
  @MainActor
  func testWaitingTask() {
    let app = XCUIApplication()
    app.launchArguments = ["-demo-server"]
    app.launch()
    XCTAssertTrue(app.buttons["tool.scout"].waitForExistence(timeout: 10))
    app.buttons["tool.scout"].tap()

    let project = app.buttons["project.upgrade-the-queue"]
    XCTAssertTrue(project.waitForExistence(timeout: 10))
    XCTAssertTrue(app.staticTexts["waiting"].exists)
    snapshot(app, "projects-waiting")
    project.tap()

    let task = app.buttons["task.02-open-the-pr"]
    XCTAssertTrue(task.waitForExistence(timeout: 10))
    task.tap()
    XCTAssertTrue(app.staticTexts["task.status"].waitForExistence(timeout: 10))
    XCTAssertEqual(app.staticTexts["task.status"].label, "Waiting")
    snapshot(app, "task-waiting")
  }

  /// Keeps a screenshot in the test results, for reviewing the screens.
  @MainActor
  private func snapshot(_ app: XCUIApplication, _ name: String) {
    let shot = XCTAttachment(screenshot: app.screenshot())
    shot.name = name
    shot.lifetime = .keepAlways
    add(shot)
  }
}

import BeagleAPI
import Foundation
import Testing

@testable import Beagle

struct TaskViewTests {
  @Test func taskTabsSplitPlanFromOutcome() throws {
    let task = try BeagleAPI.decoder.decode(TaskDetail.self, from: Fixture.data("task-done"))
    let plan = TaskView.sections(of: task, for: .task).compactMap(\.title)
    let outcome = TaskView.sections(of: task, for: .outcome).compactMap(\.title)
    #expect(plan == ["Context", "Objective", "Steps", "Done when"])
    #expect(outcome == ["Outcome", "Next steps"])
  }
}

import Foundation
import Testing

@testable import Scout

struct MarkdownTests {
  @Test func splitsAtLevelTwoHeadingsButNotInsideCode() {
    let sections = Markdown.sections(
      """
      intro

      ## Steps

      - [x] one

      ```text
      ## not a heading
      ```

      ## Outcome

      done
      """)
    #expect(sections.map(\.title) == [nil, "Steps", "Outcome"])
    #expect(sections[1].body.contains("## not a heading"))
    #expect(sections[2].body == "done")
  }

  @Test func parsesTheBlocksScoutFilesUse() {
    let blocks = Markdown.blocks(
      """
      # Title
      A paragraph
      that wraps.

      - [x] ticked
      - [ ] open
        and wrapped
      - bullet
        - nested
      2. numbered
      > quoted

      ```sh
      make check
      ```
      """)
    #expect(
      blocks == [
        .heading(level: 1, text: "Title"),
        .paragraph("A paragraph\nthat wraps."),
        .item(indent: 0, marker: "", checked: true, text: "ticked"),
        .item(indent: 0, marker: "", checked: false, text: "open and wrapped"),
        .item(indent: 0, marker: "•", checked: nil, text: "bullet"),
        .item(indent: 1, marker: "•", checked: nil, text: "nested"),
        .item(indent: 0, marker: "2.", checked: nil, text: "numbered"),
        .quote("quoted"),
        .code(language: "sh", text: "make check"),
      ])
  }

  @Test func anUnclosedFenceKeepsItsText() {
    #expect(Markdown.blocks("```\nx") == [.code(language: "", text: "x")])
  }

  @Test func spotsTemplatePlaceholders() {
    #expect(Markdown.isPlaceholder("<Filled in when the task closes.>"))
    #expect(Markdown.isPlaceholder("<What changes\nand where.>"))
    #expect(!Markdown.isPlaceholder("Uses `<T>` generics"))
  }

  @Test func taskTabsSplitPlanFromOutcome() throws {
    let data = Fixture.data("task-done")
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    let task = try decoder.decode(TaskDetail.self, from: data)
    let plan = TaskView.sections(of: task, for: .task).compactMap(\.title)
    let outcome = TaskView.sections(of: task, for: .outcome).compactMap(\.title)
    #expect(plan == ["Context", "Objective", "Steps", "Done when"])
    #expect(outcome == ["Outcome", "Next steps"])
  }
}

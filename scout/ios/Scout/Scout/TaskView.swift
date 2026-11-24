import SwiftUI

/// One task: its plan (Task tab) and its handoff (Outcome tab), like the TUI.
struct TaskView: View {
  let api: BeagleAPI
  let route: TaskRoute

  nonisolated enum Tab: String, CaseIterable, Identifiable {
    case task = "Task"
    case outcome = "Outcome"
    var id: Self { self }
  }

  /// The sections the TUI's Outcome tab shows; everything else is the Task tab.
  nonisolated static let outcomeSections: Set<String> = ["Outcome", "Next steps"]

  @State private var tab = Tab.task
  @State private var remote = Remote<TaskDetail>()

  var body: some View {
    RemoteContent(remote: remote, retry: load) { task in
      List {
        Section {
          HStack(spacing: 8) {
            TaskGlyph(status: task.status)
            Text(task.status.rawValue.capitalized)
              .foregroundStyle(task.status.color)
              .accessibilityIdentifier("task.status")
            Spacer()
            if task.checklist.total > 0 {
              ChecklistBar(done: task.checklist.done, total: task.checklist.total)
            }
          }
          if let when = ago(task.updated) {
            LabeledContent("Updated", value: when).font(.footnote)
          }
          LinksView(links: task.links)
          Picker("View", selection: $tab) {
            ForEach(Tab.allCases) { Text($0.rawValue).tag($0) }
          }
          .pickerStyle(.segmented)
          .accessibilityIdentifier("task.tabs")
        }

        let sections = Self.sections(of: task, for: tab)
        if sections.isEmpty {
          Text(tab == .outcome ? "No outcome yet" : "Nothing here yet")
            .foregroundStyle(.secondary)
        }
        ForEach(Array(sections.enumerated()), id: \.offset) { _, section in
          Section(section.title ?? "") {
            MarkdownView(text: section.body.isEmpty ? "—" : section.body)
          }
        }
      }
    }
    .navigationTitle(String(format: "%@ · %@", taskNumber, route.title))
    .navigationBarTitleDisplayMode(.inline)
    .autoRefresh(load)
  }

  private var taskNumber: String {
    String(route.task.prefix { $0.isNumber })
  }

  nonisolated static func sections(of task: TaskDetail, for tab: Tab) -> [Markdown.Section] {
    Markdown.sections(task.markdown).filter { section in
      let isOutcome = section.title.map(outcomeSections.contains) ?? false
      return tab == .outcome ? isOutcome : !isOutcome
    }
  }

  private func load() async {
    await remote.load { try await api.task(project: route.project, task: route.task) }
  }
}

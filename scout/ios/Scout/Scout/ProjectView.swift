import SwiftUI

/// One project: its tasks (with the handoff), its project.md, and its log —
/// the TUI's project screen and its Project and Log tabs.
struct ProjectView: View {
  let api: BeagleAPI
  let route: ProjectRoute

  enum Tab: String, CaseIterable, Identifiable {
    case tasks = "Tasks"
    case project = "Project"
    case log = "Log"
    var id: Self { self }
  }

  @State private var tab = Tab.tasks
  @State private var project = Remote<ProjectDetail>()
  @State private var log = Remote<LogTail>()

  var body: some View {
    VStack(spacing: 0) {
      Picker("View", selection: $tab) {
        ForEach(Tab.allCases) { Text($0.rawValue).tag($0) }
      }
      .pickerStyle(.segmented)
      .padding(.horizontal)
      .padding(.bottom, 8)
      .accessibilityIdentifier("project.tabs")

      switch tab {
      case .tasks:
        RemoteContent(remote: project, retry: load) { TasksList(project: $0) }
      case .project:
        RemoteContent(remote: project, retry: load) { detail in
          ScrollView { MarkdownView(text: detail.markdown).padding() }
        }
      case .log:
        RemoteContent(remote: log, retry: load) { LogList(log: $0) }
      }
    }
    .navigationTitle(route.title)
    .navigationBarTitleDisplayMode(.inline)
    .autoRefresh(load)
    .onChange(of: tab) { Task { await load() } }
  }

  private func load() async {
    project.apply(await Remote.fetch { try await api.project(route.id) })
    if tab == .log {
      log.apply(await Remote.fetch { try await api.log(project: route.id) })
    }
  }
}

private struct TasksList: View {
  let project: ProjectDetail

  var body: some View {
    List {
      Section {
        HStack(spacing: 8) {
          ProjectGlyph(
            status: project.status, working: project.tasks.contains { $0.status == .working })
          Text(project.status.rawValue.capitalized).foregroundStyle(project.status.color)
          Spacer()
          if let when = ago(project.lastActivity) {
            Text(when).font(.caption).foregroundStyle(.secondary)
          }
        }
        if let goal = project.goal {
          Text(goal)
        }
        LinksView(links: project.links)
      }

      if let handoff = project.handoff {
        Section {
          MarkdownView(text: handoff.markdown)
        } header: {
          Text("Next steps · from \(handoff.task)")
        }
        .accessibilityIdentifier("project.handoff")
      }

      Section("Tasks") {
        if project.tasks.isEmpty {
          Text("No tasks yet").foregroundStyle(.secondary)
        }
        ForEach(project.tasks) { task in
          NavigationLink(value: TaskRoute(project: project.id, task: task.id, title: task.title)) {
            TaskRow(task: task)
          }
          .accessibilityIdentifier("task.\(task.id)")
        }
      }

      if !project.warnings.isEmpty {
        Section("Couldn't read") {
          ForEach(project.warnings, id: \.self) { Text($0).font(.footnote) }
        }
      }
    }
  }
}

struct TaskRow: View {
  let task: TaskSummary

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: 8) {
      TaskGlyph(status: task.status)
      Text(String(format: "%02d", task.number))
        .font(.callout.monospacedDigit())
        .foregroundStyle(.secondary)
      Text(task.title)
        .fontWeight(task.status == .working ? .semibold : .regular)
        .foregroundStyle(task.status == .skipped ? Color.secondary : Color.primary)
      Spacer(minLength: 8)
      if task.checklist.total > 0 {
        ChecklistBar(done: task.checklist.done, total: task.checklist.total)
      }
    }
  }
}

/// Links (tickets, PRs, URLs): tappable when they are web addresses.
struct LinksView: View {
  let links: [String]

  var body: some View {
    ForEach(links, id: \.self) { link in
      if let url = URL(string: link), url.scheme?.hasPrefix("http") == true {
        Link(destination: url) {
          Label(link, systemImage: "link").lineLimit(1).truncationMode(.middle)
        }
      } else {
        Label(link, systemImage: "tag").foregroundStyle(.secondary)
      }
    }
    .font(.footnote)
  }
}

/// The log, newest first (the server sends it oldest first).
private struct LogList: View {
  let log: LogTail

  var body: some View {
    List {
      if log.lines.isEmpty {
        Text("Nothing logged yet").foregroundStyle(.secondary)
      }
      ForEach(Array(log.lines.enumerated().reversed()), id: \.offset) { _, line in
        inline(line.hasPrefix("- ") ? String(line.dropFirst(2)) : line)
          .font(.footnote)
      }
      if log.truncated {
        Text("Older entries are on your Mac (`log.md`).")
          .font(.caption)
          .foregroundStyle(.secondary)
      }
    }
    .listStyle(.plain)
    .accessibilityIdentifier("project.log")
  }
}

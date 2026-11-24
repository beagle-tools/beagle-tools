import SwiftUI

/// Every scout project, open ones first, like the TUI's project list.
struct ProjectListView: View {
  let api: BeagleAPI
  @State private var remote = Remote<ProjectList>()

  var body: some View {
    RemoteContent(remote: remote, retry: load) { list in
      if list.projects.isEmpty {
        ContentUnavailableView(
          "No projects yet", systemImage: "binoculars",
          description: Text("Plan one on your Mac with /scout."))
      } else {
        List {
          let open = list.projects.filter { $0.status.isOpen }
          let closed = list.projects.filter { !$0.status.isOpen }
          if !open.isEmpty {
            Section("Open") { ForEach(open) { row($0) } }
          }
          if !closed.isEmpty {
            Section("Closed") { ForEach(closed) { row($0) } }
          }
          if !list.warnings.isEmpty {
            Section("Couldn't read") {
              ForEach(list.warnings, id: \.self) { Text($0).font(.footnote) }
            }
          }
        }
        .accessibilityIdentifier("projects.list")
      }
    }
    .navigationTitle("Scout")
    .autoRefresh(load)
  }

  private func row(_ project: ProjectSummary) -> some View {
    NavigationLink(value: ProjectRoute(id: project.id, title: project.title)) {
      ProjectRow(project: project)
    }
    .accessibilityIdentifier("project.\(project.id)")
  }

  private func load() async {
    await remote.load { try await api.projects() }
  }
}

struct ProjectRow: View {
  let project: ProjectSummary

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: 10) {
      ProjectGlyph(status: project.status, working: !project.working.isEmpty)
      VStack(alignment: .leading, spacing: 4) {
        Text(project.title).font(.headline)
        if let goal = project.goal {
          Text(goal).font(.subheadline).foregroundStyle(.secondary).lineLimit(2)
        }
        ForEach(project.working, id: \.self) { task in
          HStack(spacing: 4) {
            Text("now").bold().foregroundStyle(Color.working)
            Text(task).foregroundStyle(.secondary)
          }
          .font(.caption)
        }
        HStack {
          if project.tasksTotal == 0 {
            Text("no tasks yet").font(.caption).foregroundStyle(.secondary)
          } else {
            ChecklistBar(done: project.tasksClosed, total: project.tasksTotal)
          }
          Spacer()
          if let when = ago(project.lastActivity) {
            Text(when).font(.caption).foregroundStyle(.secondary)
          }
        }
      }
    }
    .padding(.vertical, 2)
  }
}

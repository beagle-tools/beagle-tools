import SwiftUI

// Status glyphs and colours, the same as the TUI's (scout/tui/src/ui/view/mod.rs).

/// The TUI's orange for work in progress (xterm colour 208).
extension Color {
  static let working = Color(red: 1, green: 135 / 255, blue: 0)
}

extension TaskStatus {
  var glyph: String {
    switch self {
    case .todo: "○"
    case .working: "⠋"
    case .paused: "‖"
    case .blocked: "■"
    case .done: "✔"
    case .skipped: "–"
    case .unknown: "?"
    }
  }

  var color: Color {
    switch self {
    case .todo: .gray
    case .working: .working
    case .paused: .yellow
    case .blocked: .red
    case .done: .green
    case .skipped, .unknown: .secondary
    }
  }
}

extension ProjectStatus {
  var glyph: String {
    switch self {
    case .planning: "◇"
    case .active: "●"
    case .paused: "‖"
    case .done: "✔"
    case .dropped: "✖"
    case .unknown: "?"
    }
  }

  var color: Color {
    switch self {
    case .planning: .blue
    case .active: .cyan
    case .paused: .yellow
    case .done: .green
    case .dropped, .unknown: .secondary
    }
  }

  var isOpen: Bool { [.planning, .active, .paused].contains(self) }
}

/// The braille spinner every beagle TUI shows while an agent is working.
struct Spinner: View {
  static let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]

  var body: some View {
    TimelineView(.periodic(from: .now, by: 0.08)) { context in
      let tick = Int(context.date.timeIntervalSinceReferenceDate / 0.08)
      Text(Self.frames[tick % Self.frames.count])
        .foregroundStyle(Color.working)
        .bold()
    }
    .accessibilityLabel("working")
  }
}

/// A task's status glyph; spins while the task is being worked on.
struct TaskGlyph: View {
  let status: TaskStatus

  var body: some View {
    Group {
      if status == .working {
        Spinner()
      } else {
        Text(status.glyph).foregroundStyle(status.color)
          .accessibilityLabel(status.rawValue)
      }
    }
    .font(.body.monospaced())
    .frame(width: 18)
  }
}

/// A project's glyph: spins while any of its tasks is working.
struct ProjectGlyph: View {
  let status: ProjectStatus
  let working: Bool

  var body: some View {
    Group {
      if working {
        Spinner()
      } else {
        Text(status.glyph).foregroundStyle(status.color)
          .accessibilityLabel(status.rawValue)
      }
    }
    .font(.body.monospaced())
    .frame(width: 18)
  }
}

/// `done/total` with a thin green bar, like the TUI's progress line.
struct ChecklistBar: View {
  let done: Int
  let total: Int

  var body: some View {
    HStack(spacing: 6) {
      Gauge(value: Double(done), in: 0...Double(max(total, 1))) { EmptyView() }
        .gaugeStyle(.accessoryLinearCapacity)
        .tint(.green)
        .frame(width: 48)
      Text("\(done)/\(total)")
        .font(.caption.monospacedDigit())
        .foregroundStyle(.secondary)
    }
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("\(done) of \(total) done")
  }
}

/// "5 minutes ago" for an RFC 3339 timestamp, or nothing.
func ago(_ timestamp: String?) -> String? {
  Timestamp.date(timestamp)?.formatted(.relative(presentation: .named))
}

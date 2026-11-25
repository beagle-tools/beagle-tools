import BeagleAPI
import SwiftUI

extension Color {
  /// The beagle TUIs' orange for work in progress (xterm colour 208).
  public static let working = Color(red: 1, green: 135 / 255, blue: 0)
}

/// The braille spinner every beagle TUI shows while an agent is working
/// (utils/tui `SPINNER`).
public struct Spinner: View {
  /// The frames, the same as the TUIs'.
  public static let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]

  /// A spinner in the working colour.
  public init() {}

  public var body: some View {
    TimelineView(.periodic(from: .now, by: 0.08)) { context in
      let tick = Int(context.date.timeIntervalSinceReferenceDate / 0.08)
      Text(Self.frames[tick % Self.frames.count])
        .foregroundStyle(Color.working)
        .bold()
    }
    .accessibilityLabel("working")
  }
}

/// "5 minutes ago" for an RFC 3339 timestamp from the server, or nil.
public func ago(_ timestamp: String?) -> String? {
  Timestamp.date(timestamp)?.formatted(.relative(presentation: .named))
}

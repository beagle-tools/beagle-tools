import Foundation
import SwiftUI

/// Something fetched from `beagle serve`: the last good value, and the last
/// error. A failed refresh keeps showing the last value with the error above it.
@Observable
final class Remote<Value> {
  private(set) var value: Value?
  private(set) var error: String?

  func load(_ fetch: () async throws -> Value) async {
    do {
      value = try await fetch()
      error = nil
    } catch let urlError as APIError {
      if case .transport(let e) = urlError, e.code == .cancelled { return }
      error = urlError.localizedDescription
    } catch is CancellationError {
      return
    } catch {
      self.error = error.localizedDescription
    }
  }
}

/// Shows a Remote: a spinner until the first answer, the error with a retry
/// when there's nothing to show, else the content (with any newer error on top).
struct RemoteContent<Value, Content: View>: View {
  let remote: Remote<Value>
  let retry: () async -> Void
  @ViewBuilder let content: (Value) -> Content

  var body: some View {
    if let value = remote.value {
      VStack(spacing: 0) {
        if let error = remote.error {
          Label(error, systemImage: "exclamationmark.triangle")
            .font(.footnote)
            .foregroundStyle(.red)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal)
            .padding(.vertical, 6)
            .accessibilityIdentifier("remote.error")
        }
        content(value)
      }
    } else if let error = remote.error {
      ContentUnavailableView {
        Label("Can't reach your Mac", systemImage: "wifi.exclamationmark")
      } description: {
        Text(error)
      } actions: {
        Button("Try again") { Task { await retry() } }
      }
    } else {
      ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
    }
  }
}

/// Pull to refresh, plus a refresh every `interval` while the screen is shown
/// and the app is in the foreground.
struct AutoRefresh: ViewModifier {
  let interval: Duration
  let action: () async -> Void
  @Environment(\.scenePhase) private var phase

  func body(content: Content) -> some View {
    content
      .refreshable { await action() }
      .task(id: phase) {
        guard phase == .active else { return }
        while !Task.isCancelled {
          await action()
          try? await Task.sleep(for: interval)
        }
      }
  }
}

extension View {
  func autoRefresh(every interval: Duration = .seconds(10), _ action: @escaping () async -> Void)
    -> some View
  {
    modifier(AutoRefresh(interval: interval, action: action))
  }
}

/// Where the Scout tab can navigate.
struct ProjectRoute: Hashable {
  let id: String
  let title: String
}

struct TaskRoute: Hashable {
  let project: String
  let task: String
  let title: String
}

import SwiftUI

/// Renders scout Markdown natively: blocks from `Markdown.blocks`, inline
/// styling through AttributedString. Template placeholders are dimmed, as in the TUI.
struct MarkdownView: View {
  let text: String

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      ForEach(Array(Markdown.blocks(text).enumerated()), id: \.offset) { _, block in
        BlockView(block: block)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .textSelection(.enabled)
  }
}

/// Inline Markdown (bold, italics, `code`, links) as styled text.
func inline(_ text: String) -> Text {
  if Markdown.isPlaceholder(text) {
    return Text(text).italic().foregroundStyle(.secondary)
  }
  let options = AttributedString.MarkdownParsingOptions(
    interpretedSyntax: .inlineOnlyPreservingWhitespace)
  if let styled = try? AttributedString(markdown: text, options: options) {
    return Text(styled)
  }
  return Text(text)
}

private struct BlockView: View {
  let block: Markdown.Block

  var body: some View {
    switch block {
    case .heading(let level, let text):
      inline(text)
        .font(level <= 2 ? .headline : .subheadline.weight(.semibold))
        .padding(.top, 4)
    case .paragraph(let text):
      inline(text)
    case .item(let indent, let marker, let checked, let text):
      HStack(alignment: .firstTextBaseline, spacing: 6) {
        if let checked {
          Image(systemName: checked ? "checkmark.square.fill" : "square")
            .foregroundStyle(checked ? Color.green : Color.secondary)
            .accessibilityLabel(checked ? "done" : "not done")
        } else {
          Text(marker).foregroundStyle(.secondary).monospacedDigit()
        }
        inline(text)
          .foregroundStyle(checked == true ? Color.secondary : Color.primary)
      }
      .padding(.leading, CGFloat(indent) * 16)
    case .quote(let text):
      inline(text)
        .foregroundStyle(.secondary)
        .padding(.leading, 10)
        .overlay(alignment: .leading) {
          Rectangle().fill(.tertiary).frame(width: 3)
        }
    case .code(_, let text):
      ScrollView(.horizontal, showsIndicators: false) {
        Text(text)
          .font(.system(.caption, design: .monospaced))
          .fixedSize()
          .padding(10)
      }
      .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8))
    }
  }
}

#Preview {
  ScrollView {
    MarkdownView(
      text: """
        ## Steps

        - [x] Add the **migration**
        - [ ] Load the list at `startup`
        1. numbered
        > a quote

        ```text
        client ──▶ edge ──▶ handler
        ```

        <Filled in when the task closes.>
        """
    )
    .padding()
  }
}

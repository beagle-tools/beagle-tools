import Foundation

/// A small Markdown block parser for scout's files: enough for headings,
/// paragraphs, checkbox and bullet lists, numbered lists, quotes and code
/// fences. Inline styling (bold, code, links) is left to AttributedString.
nonisolated enum Markdown {
  enum Block: Equatable, Sendable {
    case heading(level: Int, text: String)
    case paragraph(String)
    /// `checked` is nil for a plain bullet, true/false for `- [x]` / `- [ ]`.
    case item(indent: Int, marker: String, checked: Bool?, text: String)
    case quote(String)
    case code(language: String, text: String)
  }

  /// A `## Title` section of a scout file and the Markdown under it.
  struct Section: Equatable, Sendable {
    let title: String?
    let body: String
  }

  /// Splits a document at its `## ` headings. Text before the first one is a
  /// section with no title. Headings inside code fences don't split.
  static func sections(_ text: String) -> [Section] {
    var sections: [Section] = []
    var title: String?
    var lines: [Substring] = []
    var fenced = false
    func flush() {
      let body = lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
      if title != nil || !body.isEmpty { sections.append(Section(title: title, body: body)) }
      lines = []
    }
    for line in text.split(separator: "\n", omittingEmptySubsequences: false) {
      if line.hasPrefix("```") { fenced.toggle() }
      if !fenced, line.hasPrefix("## ") {
        flush()
        title = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
      } else {
        lines.append(line)
      }
    }
    flush()
    return sections
  }

  /// True for scout's unfilled template text, e.g. `<Filled in when …>`.
  static func isPlaceholder(_ text: String) -> Bool {
    let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
    return t.hasPrefix("<") && t.hasSuffix(">") && !t.contains("](")
  }

  static func blocks(_ text: String) -> [Block] {
    var blocks: [Block] = []
    var paragraph: [String] = []
    var code: (language: String, lines: [String])?

    func flushParagraph() {
      if !paragraph.isEmpty { blocks.append(.paragraph(paragraph.joined(separator: "\n"))) }
      paragraph = []
    }

    for raw in text.split(separator: "\n", omittingEmptySubsequences: false) {
      let line = String(raw)
      if var open = code {
        if line.trimmingCharacters(in: .whitespaces).hasPrefix("```") {
          blocks.append(.code(language: open.language, text: open.lines.joined(separator: "\n")))
          code = nil
        } else {
          open.lines.append(line)
          code = open
        }
        continue
      }
      let trimmed = line.trimmingCharacters(in: .whitespaces)
      if trimmed.hasPrefix("```") {
        flushParagraph()
        code = (String(trimmed.dropFirst(3)).trimmingCharacters(in: .whitespaces), [])
      } else if trimmed.isEmpty {
        flushParagraph()
      } else if let heading = heading(trimmed) {
        flushParagraph()
        blocks.append(heading)
      } else if let item = item(line) {
        flushParagraph()
        blocks.append(item)
      } else if trimmed.hasPrefix(">") {
        flushParagraph()
        let quoted = String(trimmed.dropFirst()).trimmingCharacters(in: .whitespaces)
        if case .quote(let previous)? = blocks.last {
          blocks[blocks.count - 1] = .quote(previous + "\n" + quoted)
        } else {
          blocks.append(.quote(quoted))
        }
      } else if case .item(let indent, let marker, let checked, let text)? = blocks.last,
        paragraph.isEmpty, line.hasPrefix("  ")
      {
        // A wrapped list item continues on an indented line.
        blocks[blocks.count - 1] = .item(
          indent: indent, marker: marker, checked: checked, text: text + " " + trimmed)
      } else {
        paragraph.append(trimmed)
      }
    }
    if let open = code {
      blocks.append(.code(language: open.language, text: open.lines.joined(separator: "\n")))
    }
    flushParagraph()
    return blocks
  }

  private static func heading(_ line: String) -> Block? {
    let hashes = line.prefix { $0 == "#" }.count
    guard (1...6).contains(hashes), line.dropFirst(hashes).first == " " else { return nil }
    return .heading(level: hashes, text: String(line.dropFirst(hashes + 1)))
  }

  private static func item(_ line: String) -> Block? {
    let indent = line.prefix { $0 == " " }.count / 2
    let rest = line.drop { $0 == " " }
    var marker: String
    var text: Substring
    if let first = rest.first, "-*+".contains(first), rest.dropFirst().first == " " {
      marker = "•"
      text = rest.dropFirst(2)
    } else {
      let digits = rest.prefix { $0.isNumber }
      guard !digits.isEmpty, rest.dropFirst(digits.count).hasPrefix(". ") else { return nil }
      marker = "\(digits)."
      text = rest.dropFirst(digits.count + 2)
    }
    var checked: Bool?
    if marker == "•" {
      if text.hasPrefix("[ ] ") || text == "[ ]" {
        checked = false
      } else if text.lowercased().hasPrefix("[x] ") || text.lowercased() == "[x]" {
        checked = true
      }
      if checked != nil {
        text = text.dropFirst(min(4, text.count))
        marker = ""
      }
    }
    return .item(indent: indent, marker: marker, checked: checked, text: String(text))
  }
}

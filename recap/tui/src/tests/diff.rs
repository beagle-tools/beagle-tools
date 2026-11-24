//! Tests for `diff`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

const SAMPLE: &str = "\
diff --git a/router.yaml b/router.yaml
index 111..222 100644
--- a/router.yaml
+++ b/router.yaml
@@ -80,6 +80,8 @@ spec:
   livenessProbe:
+    timeoutSeconds: 5
\tperiodSeconds: 15
-    old: line
diff --git a/new.txt b/new.txt
new file mode 100644
--- /dev/null
+++ b/new.txt
@@ -0,0 +1 @@
+hello
\\ No newline at end of file
diff --git a/old.md b/renamed.md
similarity index 90%
rename from old.md
rename to renamed.md
";

fn text(l: &DocLine) -> String {
    crate::wrap::plain_text(l)
}

#[test]
fn stats_count_each_file() {
    let s = stats(SAMPLE);
    assert_eq!(s.len(), 3);
    assert_eq!(
        (s[0].path.as_str(), s[0].added, s[0].removed),
        ("router.yaml", 1, 1)
    );
    assert_eq!((s[1].note.as_str(), s[1].added), ("new", 1));
    assert_eq!(
        (s[2].path.as_str(), s[2].note.as_str()),
        ("renamed.md", "renamed from old.md")
    );
}

#[test]
fn render_has_summary_headers_and_jump_points() {
    let r = render(SAMPLE);
    assert!(text(&r.lines[0]).starts_with("3 files changed  +2 −1"));
    assert_eq!(r.file_starts.len(), 3);
    assert!(text(&r.lines[r.file_starts[0]]).starts_with("━━ router.yaml +1 −1"));
    assert!(r.lines.iter().all(|l| l.pre), "diff lines never wrap");
    let all: Vec<String> = r.lines.iter().map(text).collect();
    assert!(
        all.iter().any(|l| l == "    periodSeconds: 15"),
        "tabs expanded"
    );
    assert!(!all
        .iter()
        .any(|l| l.starts_with("index ") || l.starts_with("+++ ")));
    let plus = r
        .lines
        .iter()
        .find(|l| text(l) == "+    timeoutSeconds: 5")
        .unwrap();
    assert_eq!(plus.line.style.fg, Some(Color::Green));
}

#[test]
fn empty_diff_explains_itself() {
    let r = render("");
    assert_eq!(r.file_starts.len(), 0);
    assert!(text(&r.lines[0]).contains("recap refresh"));
}

#[test]
fn delta_output_keeps_colours_and_finds_file_headers() {
    // Shaped like `delta --side-by-side` output: path, rule, hunk box, rows.
    let output = "\n\u{1b}[34mrouter.yaml\u{1b}[0m\n\u{1b}[34m──────────\u{1b}[0m\n\
        │ 82 │  readinessProbe:      │ 82 │  readinessProbe:\n\
        │    │                       │ 85 │\u{1b}[48;2;0;40;0m    timeoutSeconds: 5\u{1b}[0m\n\
        \n\u{1b}[34mnew.txt\u{1b}[0m\n\u{1b}[34m──────────\u{1b}[0m\n\
        │    │                       │  1 │\u{1b}[48;2;0;40;0mhello\u{1b}[0m\n";
    let r = from_delta_output(SAMPLE, output);
    assert!(text(&r.lines[0]).starts_with("3 files changed"));
    assert_eq!(
        r.file_starts.len(),
        2,
        "renamed.md has no hunk in this output"
    );
    assert_eq!(text(&r.lines[r.file_starts[0]]), "router.yaml");
    assert_eq!(text(&r.lines[r.file_starts[1]]), "new.txt");
    assert!(r.lines.iter().all(|l| l.pre));
    let added = r
        .lines
        .iter()
        .find(|l| text(l).contains("timeoutSeconds"))
        .unwrap();
    let bg = added
        .line
        .spans
        .iter()
        .find(|s| s.content.contains("timeoutSeconds"))
        .unwrap()
        .style
        .bg;
    assert_eq!(
        bg,
        Some(Color::Rgb(0, 40, 0)),
        "delta's added-line background survives"
    );
}

#[test]
fn real_delta_renders_side_by_side_when_installed() {
    if !delta_available() {
        return; // nothing to check on a machine without delta
    }
    let r = render_delta(SAMPLE, 100).unwrap();
    let all: Vec<String> = r.lines.iter().map(text).collect();
    assert!(
        all.iter()
            .any(|l| l.contains("timeoutSeconds: 5") && l.matches('│').count() >= 3),
        "{all:#?}"
    );
    assert_ne!(r.file_starts, Vec::<usize>::new());
}

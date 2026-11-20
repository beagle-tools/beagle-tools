// Tests panic on a broken fixture; that is the failure mode we want.
#![allow(clippy::unwrap_used)]

use super::*;
use tandem_core::diff_parse::parse_unified_diff;

const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs
index 1..2 100644
--- a/src/a.rs
+++ b/src/a.rs
@@ -1,3 +1,4 @@
 fn a() {
-    old();
+    new();
+    more();
 }
";

fn diff() -> Vec<FileDiff> {
    parse_unified_diff(DIFF).unwrap()
}

const VERDICT: &str =
    r#"{"type":"tandem_verdict","verdict":"mergeable-with-suggestions","reason":"one nit"}"#;
const SUMMARY: &str = r#"{"type":"tandem_summary","body":"1. What is the problem"}"#;

fn comment(extra: &str) -> String {
    format!(
        r#"{{"type":"tandem_comment","path":"src/a.rs","side":"new","line":2,"severity":"suggestion","tag":"naming","body":"rename","suggestion":null,"context":"why"{extra}}}"#
    )
}

#[test]
fn a_good_review_parses() {
    let text = format!("{VERDICT}\n{SUMMARY}\n\n{}\n", comment(""));
    let review = parse(&text, &diff(), false).unwrap();
    assert_eq!(review.verdict, Verdict::WithSuggestions);
    assert_eq!(review.summary.as_deref(), Some("1. What is the problem"));
    assert_eq!(review.comments.len(), 1);
    assert_eq!(review.comments[0].line, 2);
}

#[test]
fn an_existing_summary_means_none_is_wanted() {
    let text = format!("{VERDICT}\n{}", comment(""));
    assert!(parse(&text, &diff(), true).unwrap().summary.is_none());
    let errors = parse(&format!("{VERDICT}\n{SUMMARY}"), &diff(), true).unwrap_err();
    assert!(errors[0].contains("already exists"), "{errors:?}");
    let errors = parse(VERDICT, &diff(), false).unwrap_err();
    assert!(errors[0].contains("tandem_summary"), "{errors:?}");
}

#[test]
fn every_problem_is_reported_with_its_line() {
    let off_diff = comment("").replace(r#""line":2"#, r#""line":40"#);
    let no_context = comment("").replace(r#""context":"why""#, r#""context":" ""#);
    let dup = comment(r#","duplicate_of":null"#).replace("naming", "duplicate");
    let text = format!("{SUMMARY}\n{off_diff}\n{no_context}\n{dup}\nnot json");
    let errors = parse(&text, &diff(), false).unwrap_err();
    let all = errors.join("\n");
    assert!(
        all.contains("line 2: src/a.rs:40 is not on the new side"),
        "{all}"
    );
    assert!(all.contains("line 3: the context"), "{all}");
    assert!(
        all.contains("line 4: a duplicate needs duplicate_of"),
        "{all}"
    );
    assert!(all.contains("line 5: not a valid review line"), "{all}");
    assert!(all.contains("exactly one tandem_verdict"), "{all}");
}

#[test]
fn sides_and_pr_level_comments() {
    // Old line 2 was removed: anchorable on the old side, not the new.
    let old = comment("").replace(r#""side":"new""#, r#""side":"old""#);
    assert!(parse(&format!("{VERDICT}\n{old}"), &diff(), true).is_ok());
    let pr_level = comment("")
        .replace(r#""path":"src/a.rs""#, r#""path":"""#)
        .replace(r#""line":2"#, r#""line":0"#);
    assert!(parse(&format!("{VERDICT}\n{pr_level}"), &diff(), true).is_ok());
    let missing = comment("").replace("src/a.rs", "src/b.rs");
    let errors = parse(&format!("{VERDICT}\n{missing}"), &diff(), true).unwrap_err();
    assert!(errors[0].contains("not a file in this diff"), "{errors:?}");
}

#[test]
fn unknown_fields_and_verdicts_are_refused() {
    let typo = comment(r#","sugestion":"x""#);
    let errors = parse(&format!("{VERDICT}\n{typo}"), &diff(), true).unwrap_err();
    assert!(errors[0].contains("unknown field"), "{errors:?}");
    let bad = VERDICT.replace("mergeable-with-suggestions", "lgtm");
    let errors = parse(&bad, &diff(), true).unwrap_err();
    assert!(errors[0].contains("unknown verdict"), "{errors:?}");
}

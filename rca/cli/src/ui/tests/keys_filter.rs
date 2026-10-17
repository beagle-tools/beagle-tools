//! Key-handling tests for filter mode (facets + fuzzy query), the sidebar
//! collapse, and the archived toggle (`ui::keys`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crate::ui::testutil::{app_with, press};

use super::*;

#[test]
fn f_filter_narrows_the_list_and_esc_peels() {
    let mut app = app_with(3); // titles "RCA 0".."RCA 2"
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(app.visible_len(), 1);
    assert_eq!(app.selected_rca().map(|r| r.id.as_str()), Some("rca-2"));

    press(&mut app, KeyCode::Esc);
    assert!(app.search_active(), "first esc only stops typing");
    assert_eq!(app.filter(), "2", "query survives the peel");

    press(&mut app, KeyCode::Esc);
    assert!(!app.search_active());
    assert_eq!(app.filter(), "");
    assert_eq!(app.visible_len(), 3, "second esc restores the full list");
}

/// Workspaces spanning severities and statuses, for facet tests.
fn app_with_variety() -> crate::ui::App {
    use crate::model::{RcaId, Severity, Status};
    use crate::store::{new_meta, Store};

    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("store");
    let scaffold = |slug: &str, title: &str, severity: Severity, status: Option<Status>| {
        let id = RcaId::new(slug).expect("valid id");
        store
            .scaffold(&id, &new_meta(title.to_owned(), severity))
            .expect("scaffold");
        if let Some(status) = status {
            store.set_status(&id, status).expect("status");
        }
    };
    scaffold(
        "alloy-inv-high",
        "Alloy pool exhausted",
        Severity::High,
        None,
    );
    scaffold("cron-inv-low", "Cron drift", Severity::Low, None);
    scaffold(
        "ses-rev-high",
        "SES blocklist",
        Severity::High,
        Some(Status::Review),
    );
    scaffold(
        "rds-fin-med",
        "RDS locks",
        Severity::Medium,
        Some(Status::Finished),
    );
    std::mem::forget(tmp); // OS cleans the temp root; see app_with
    crate::ui::App::new(store).expect("app")
}

#[test]
fn facet_keys_stack_toggle_and_never_type() {
    let mut app = app_with_variety();
    press(&mut app, KeyCode::Char('f'));

    press(&mut app, KeyCode::Char('h')); // severity: high
    assert_eq!(app.visible_len(), 2, "two high-severity incidents");
    assert!(app.filter().is_empty(), "facet keys do not type");
    assert_eq!(app.facet_label(), "[high]");

    press(&mut app, KeyCode::Char('i')); // + status: investigating
    assert_eq!(app.visible_len(), 1, "facets AND across dimensions");
    assert_eq!(
        app.selected_rca().expect("match").id.as_str(),
        "alloy-inv-high"
    );
    assert_eq!(app.facet_label(), "[high · investigating]");

    press(&mut app, KeyCode::Char('h')); // toggle high off
    assert_eq!(app.visible_len(), 2, "both investigating incidents return");

    press(&mut app, KeyCode::Char('v')); // + final-review (none exist)
    press(&mut app, KeyCode::Char('i')); // investigating off → only v left
    assert_eq!(app.visible_len(), 0, "no final-review incidents");

    press(&mut app, KeyCode::Esc); // facet mode esc clears everything
    assert!(!app.has_active_filter());
    assert_eq!(app.visible_len(), 4);
}

#[test]
fn facets_combine_with_free_text_and_survive_enter() {
    let mut app = app_with_variety();
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('i')); // investigating: 2 left
    press(&mut app, KeyCode::Char('/')); // switch to typing
    for c in "cron".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.visible_len(), 1, "text ranks within the facet set");
    assert_eq!(
        app.selected_rca().expect("match").id.as_str(),
        "cron-inv-low"
    );

    press(&mut app, KeyCode::Enter); // keep everything, leave filter mode
    assert!(!app.search_active());
    assert!(app.has_active_filter(), "facets + text survive enter");

    press(&mut app, KeyCode::Enter); // open the incident → consumed
    assert!(!app.has_active_filter(), "opening consumes facets too");
    assert_eq!(app.visible_len(), 4);
    assert_eq!(
        app.selected_rca().expect("selected").id.as_str(),
        "cron-inv-low"
    );
}

#[test]
fn typing_q_in_filter_mode_filters_instead_of_quitting() {
    let mut app = app_with(2);
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(press(&mut app, KeyCode::Char('q')), Flow::Continue);
    assert_eq!(app.filter(), "q");
    assert_eq!(app.visible_len(), 0, "no workspace matches `q`");
    // Backspace repairs the query.
    press(&mut app, KeyCode::Backspace);
    assert_eq!(app.visible_len(), 2);
}

#[test]
fn enter_keeps_filter_and_esc_in_list_mode_clears_it() {
    let mut app = app_with(3);
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('1'));
    press(&mut app, KeyCode::Enter);
    assert!(!app.search_active());
    assert_eq!(app.visible_len(), 1, "filter survives enter");

    press(&mut app, KeyCode::Esc); // list-mode esc clears a kept filter
    assert_eq!(app.visible_len(), 3);
}

#[test]
fn opening_an_incident_consumes_the_filter_and_restores_the_list() {
    let mut app = app_with(3); // titles "RCA 0".."RCA 2"
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('2'));
    press(&mut app, KeyCode::Enter); // commit the filter
    assert_eq!(app.visible_len(), 1);
    let picked = app.selected_rca().expect("match").id.clone();

    press(&mut app, KeyCode::Enter); // open the incident
    assert_eq!(app.focus(), Focus::Content);
    assert!(app.filter().is_empty(), "the pick consumed the filter");
    assert_eq!(app.visible_len(), 3, "everyone is back");
    assert_eq!(
        app.selected_rca().expect("selected").id,
        picked,
        "selection stays on the incident you opened"
    );
}

#[test]
fn p_facet_filters_to_incidents_with_a_pr() {
    use crate::model::RcaId;

    let mut app = app_with_variety();
    let id = RcaId::new("ses-rev-high").expect("id");
    app.store
        .add_pr(&id, "https://github.com/o/r/pull/1")
        .expect("pr");
    app.reload();
    let total = app.visible_len();

    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('p')); // has-PR facet
    assert_eq!(app.visible_len(), 1, "only the incident with a PR");
    assert_eq!(
        app.selected_rca().expect("match").id.as_str(),
        "ses-rev-high"
    );
    assert_eq!(app.facet_label(), "[has PR]");
    assert!(app.filter().is_empty(), "p does not type");

    press(&mut app, KeyCode::Char('p')); // toggle off
    assert_eq!(app.visible_len(), total, "all incidents return");
    assert_eq!(app.facet_label(), "");
}

#[test]
fn s_facet_filters_to_security_tagged_incidents() {
    let mut app = app_with_variety();
    // Tag one incident `security`.
    let id = crate::model::RcaId::new("ses-rev-high").expect("id");
    app.store
        .set_tags(&id, vec!["security".to_owned()])
        .expect("tag");
    app.reload();
    let total = app.visible_len();

    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('s')); // security facet
    assert_eq!(app.visible_len(), 1, "only the security-tagged incident");
    assert_eq!(
        app.selected_rca().expect("match").id.as_str(),
        "ses-rev-high"
    );
    assert_eq!(app.facet_label(), "[security]");

    press(&mut app, KeyCode::Char('s')); // toggle off
    assert_eq!(app.visible_len(), total);
    assert_eq!(app.facet_label(), "");
}

#[test]
fn s_collapses_the_sidebar_and_focuses_content() {
    let mut app = app_with(1);
    assert!(!app.sidebar_collapsed());
    press(&mut app, KeyCode::Char('s'));
    assert!(app.sidebar_collapsed());
    assert_eq!(
        app.focus(),
        Focus::Content,
        "a hidden list cannot hold the cursor"
    );
    press(&mut app, KeyCode::Char('s'));
    assert!(!app.sidebar_collapsed(), "s toggles back");
    assert_eq!(
        app.focus(),
        Focus::Content,
        "expanding does not steal focus"
    );
}

#[test]
fn returning_focus_to_the_list_expands_the_sidebar() {
    // Every back-to-list path must bring the sidebar back: b, esc, f.
    for code in [KeyCode::Char('b'), KeyCode::Esc, KeyCode::Char('f')] {
        let mut app = app_with(1);
        press(&mut app, KeyCode::Char('s'));
        assert!(app.sidebar_collapsed());
        press(&mut app, code);
        assert_eq!(app.focus(), Focus::List, "{code:?} should focus the list");
        assert!(
            !app.sidebar_collapsed(),
            "{code:?} should expand the sidebar"
        );
    }
}

#[test]
fn collapsing_on_an_empty_store_does_not_panic() {
    let mut app = app_with(0);
    press(&mut app, KeyCode::Char('s'));
    assert!(app.sidebar_collapsed());
    press(&mut app, KeyCode::Esc);
    assert!(!app.sidebar_collapsed());
}

#[test]
fn a_toggles_archived_incidents_into_the_list() {
    let mut app = app_with(2);
    let id = crate::model::RcaId::new("rca-1").expect("id");
    app.store
        .set_status(&id, crate::model::Status::Finished)
        .expect("finish");
    app.store.archive(&id, false).expect("archive");
    app.reload();

    assert_eq!(app.visible_len(), 1, "archived hidden by default");
    assert_eq!(app.archived_count(), 1);

    press(&mut app, KeyCode::Char('a'));
    assert_eq!(app.visible_len(), 2, "a shows archived");
    let last = app.visible_rcas().last().expect("row");
    assert!(last.archived, "archived sorts below active");

    press(&mut app, KeyCode::Char('a'));
    assert_eq!(app.visible_len(), 1, "a hides them again");
}

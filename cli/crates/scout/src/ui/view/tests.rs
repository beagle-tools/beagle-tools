//! Drawing tests against a `TestBackend`.

// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::pane::*;
use super::*;
use crate::model::Slug;
use crate::store::Store;
use ratatui::backend::TestBackend;
use ratatui::text::Text;
use ratatui::Terminal;

fn render(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    app.ensure_pane();
    terminal.draw(|frame| draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    buffer
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn empty_store_renders_a_hint() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Store::new(dir.path())).unwrap();
    assert!(render(&mut app).contains("No projects yet."));
}

#[test]
fn both_screens_render_the_working_task() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    store
        .create_project(&Slug::parse("p").unwrap(), "Lock down ingress", vec![])
        .unwrap();
    store
        .add_task(
            "p",
            &Slug::parse("map").unwrap(),
            "Map the request path",
            vec![],
        )
        .unwrap();
    store
        .set_task_status("p", "1", TaskStatus::Working)
        .unwrap();
    let mut app = App::new(store).unwrap();
    let projects = render(&mut app);
    assert!(projects.contains("Lock down ingress"));
    assert!(projects.contains("now"));
    app.open_project();
    let tasks = render(&mut app);
    assert!(tasks.contains("01 Map the request path"));
    assert!(tasks.contains("1 Task"));
    app.help = true;
    assert!(render(&mut app).contains("any key closes this"));
}

/// Drawing only the visible lines must look exactly like drawing them
/// all and scrolling, at every offset, including mid-way through a
/// wrapped line.
#[test]
fn virtualized_scrolling_matches_a_full_render() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    store
        .create_project(&Slug::parse("p").unwrap(), "P", vec![])
        .unwrap();
    store
        .add_task("p", &Slug::parse("a").unwrap(), "A", vec![])
        .unwrap();
    let mut app = App::new(store).unwrap();
    app.open_project();
    app.tab = Tab::Log;
    app.ensure_pane();
    let lines: Vec<Line<'static>> = (0..300)
        .map(|i| Line::raw(format!("line {i} {}", "word ".repeat(i % 23))))
        .collect();
    let pane = app.pane.as_mut().unwrap();
    pane.text = Text::from(lines.clone());
    pane.layout = None;
    let area = Rect::new(0, 0, 40, 12);
    for scroll in [0, 1, 5, 37, 200, 555, usize::MAX] {
        app.scroll = scroll;
        let mut fast = Terminal::new(TestBackend::new(40, 12)).unwrap();
        fast.draw(|f| {
            render_scrolled(
                f,
                &mut app,
                area,
                pane_block(String::new(), false),
                Vec::new(),
            );
        })
        .unwrap();
        let clamped = u16::try_from(app.scroll).unwrap();
        let mut full = Terminal::new(TestBackend::new(40, 12)).unwrap();
        full.draw(|f| {
            let p = Paragraph::new(Text::from(lines.clone()))
                .wrap(Wrap { trim: false })
                .scroll((clamped, 0))
                .block(pane_block(String::new(), false));
            f.render_widget(p, area);
        })
        .unwrap();
        assert_eq!(
            fast.backend().buffer(),
            full.backend().buffer(),
            "scroll {scroll}"
        );
    }
    assert!(app.max_scroll > 300, "wrapped rows, not lines");
}

#[test]
fn layout_extends_on_append_and_resets_on_edit() {
    let old: Vec<Line<'static>> = vec![Line::raw("a"), Line::raw("word ".repeat(20))];
    let base = layout(&old, 20);
    let mut grown = old.clone();
    grown.push(Line::raw("c"));
    let extended = extend_layout(layout(&old, 20), &old, &grown).unwrap();
    assert_eq!(extended.starts, layout(&grown, 20).starts);
    assert_eq!(extended.starts[..base.starts.len()], base.starts[..]);
    let edited = vec![Line::raw("changed"), Line::raw("b")];
    assert!(extend_layout(layout(&old, 20), &old, &edited).is_none());
    assert!(extend_layout(layout(&old, 20), &old, &old[..1]).is_none());
}

#[test]
fn truncation_and_activity() {
    assert_eq!(truncate("abcdef", 4), "abc…");
    assert_eq!(truncate("abc", 4), "abc");
    let (label, quiet) = activity(Some(SystemTime::now())).unwrap();
    assert_eq!((label.as_str(), quiet), ("active 0m ago", false));
    let old = SystemTime::now() - Duration::from_mins(75);
    assert_eq!(
        activity(Some(old)).unwrap(),
        ("quiet 1h15m".to_owned(), true)
    );
}

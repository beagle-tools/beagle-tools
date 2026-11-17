//! Tests for saving agent specs — a rename moves the spec, never clones
//! it, and never overwrites another agent.

#![allow(clippy::unwrap_used)]

use tandem_cache::{Cache, ReviewStore};
use tandem_core::agent::AgentSpec;

use super::save_spec;

fn spec(name: &str, prompt: &str) -> AgentSpec {
    serde_json::from_value(serde_json::json!({
        "name": name,
        "runner": { "kind": "claude_headless" },
        "auth": { "kind": "cli_session" },
        "model": null,
        "append_system_prompt": null,
        "prompt": prompt,
    }))
    .unwrap()
}

fn names(cache: &Cache) -> Vec<String> {
    cache
        .list_agent_specs()
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect()
}

#[test]
fn rename_moves_the_spec() {
    let cache = Cache::open_in_memory().unwrap();
    save_spec(&cache, &spec("old", "p"), None).unwrap();
    save_spec(&cache, &spec("new", "p2"), Some("old")).unwrap();
    assert_eq!(names(&cache), vec!["new".to_owned()]);
}

#[test]
fn rename_onto_an_existing_agent_is_refused() {
    let cache = Cache::open_in_memory().unwrap();
    save_spec(&cache, &spec("a", "p"), None).unwrap();
    save_spec(&cache, &spec("b", "p"), None).unwrap();
    let err = save_spec(&cache, &spec("b", "clobber"), Some("a")).unwrap_err();
    assert!(err.to_string().contains("already an agent called b"));
    assert_eq!(names(&cache), vec!["a".to_owned(), "b".to_owned()]);
    let b = cache
        .list_agent_specs()
        .unwrap()
        .into_iter()
        .find(|s| s.name == "b")
        .unwrap();
    assert_eq!(b.prompt, "p");
}

#[test]
fn same_name_or_new_spec_is_an_upsert() {
    let cache = Cache::open_in_memory().unwrap();
    save_spec(&cache, &spec("a", "p"), None).unwrap();
    save_spec(&cache, &spec("a", "p2"), Some("a")).unwrap();
    save_spec(&cache, &spec("c", "p"), Some("")).unwrap();
    assert_eq!(names(&cache), vec!["a".to_owned(), "c".to_owned()]);
}

#[test]
fn empty_name_is_refused() {
    let cache = Cache::open_in_memory().unwrap();
    assert!(save_spec(&cache, &spec("  ", "p"), None).is_err());
}

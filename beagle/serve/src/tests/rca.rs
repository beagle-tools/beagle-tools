// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use super::*;

/// The example RCA shipped in the repo (`rca/rcas/`), read in place.
const EXAMPLE: &str = "2026-05-12-example-checkout-timeouts";

fn repo_rca_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rca")
}

fn get(root: &Path, path: &str) -> ApiResult {
    let req = Request::get(path, None);
    let rest: Vec<&str> = req
        .path
        .split('/')
        .filter(|s| !s.is_empty())
        .skip(2)
        .collect();
    route(root, &rest, &req)
}

/// A temp store: the example (active), a copy of it archived, and a
/// directory with a broken manifest.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let rcas = dir.path().join("rcas");
    let src = repo_rca_root().join("rcas").join(EXAMPLE);
    copy_dir(&src, &rcas.join(EXAMPLE));
    copy_dir(&src, &rcas.join("archive/2026-01-01-old-one"));
    std::fs::create_dir_all(rcas.join("half-written")).unwrap();
    std::fs::write(rcas.join("half-written/rca.toml"), "title = 1\n").unwrap();
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

#[test]
fn the_list_matches_rca_list_json() {
    let dir = fixture();
    let body = get(dir.path(), "/v1/rca").unwrap();
    let listing = Store::open(dir.path()).unwrap().list_all().unwrap();
    let summaries: Vec<&RcaSummary> = listing.summaries.iter().collect();
    let cli: Value = serde_json::from_str(&beagle_rca::list_json(&summaries).unwrap()).unwrap();
    assert_eq!(body["rcas"], cli);
    let rcas = body["rcas"].as_array().unwrap();
    assert_eq!(rcas.len(), 2);
    assert_eq!(rcas[0]["id"], EXAMPLE);
    assert_eq!(rcas[0]["archived"], false);
    assert_eq!(rcas[1]["archived"], true);
    assert_eq!(body["broken"][0]["dir"], "half-written");
}

#[test]
fn one_rca_lists_its_sections_and_diagrams() {
    let root = repo_rca_root();
    let body = get(&root, &format!("/v1/rca/{EXAMPLE}")).unwrap();
    assert_eq!(body["rca"]["id"], EXAMPLE);
    let sections = body["sections"].as_array().unwrap();
    assert_eq!(sections.len(), SectionKind::ALL.len());
    assert_eq!(sections[0]["name"], "summary");
    assert_eq!(sections[0]["present"], true);
    let commands = sections.iter().find(|s| s["name"] == "commands").unwrap();
    assert_eq!(commands["present"], false);
    assert_eq!(body["diagrams"], json!(["01-request-path.txt"]));
}

#[test]
fn a_section_is_the_file_verbatim() {
    let root = repo_rca_root();
    let body = get(&root, &format!("/v1/rca/{EXAMPLE}/sections/root-cause")).unwrap();
    let file =
        std::fs::read_to_string(root.join("rcas").join(EXAMPLE).join("root-cause.md")).unwrap();
    assert_eq!(body["markdown"], file);
    assert_eq!(body["title"], "Root Cause");
    let missing = get(&root, &format!("/v1/rca/{EXAMPLE}/sections/commands")).unwrap_err();
    assert_eq!(missing.status, 404);
    let unknown = get(&root, &format!("/v1/rca/{EXAMPLE}/sections/nope")).unwrap_err();
    assert_eq!(unknown.status, 404);
}

#[test]
fn a_diagram_by_index_or_name() {
    let root = repo_rca_root();
    let by_index = get(&root, &format!("/v1/rca/{EXAMPLE}/diagrams/1")).unwrap();
    let by_name = get(
        &root,
        &format!("/v1/rca/{EXAMPLE}/diagrams/01-request-path.txt"),
    )
    .unwrap();
    assert_eq!(by_index, by_name);
    assert!(!by_index["text"].as_str().unwrap().contains('\u{1b}'));
    for bad in ["0", "2", "nope.txt"] {
        let err = get(&root, &format!("/v1/rca/{EXAMPLE}/diagrams/{bad}")).unwrap_err();
        assert_eq!(err.status, 404, "{bad}");
    }
}

#[test]
fn archived_rcas_are_served_too() {
    let dir = fixture();
    let body = get(dir.path(), "/v1/rca/2026-01-01-old-one").unwrap();
    assert_eq!(body["rca"]["archived"], true);
    assert!(get(dir.path(), "/v1/rca/2026-01-01-old-one/sections/summary").is_ok());
}

#[test]
fn bad_ids_are_400_and_unknown_ones_404() {
    let root = repo_rca_root();
    assert_eq!(get(&root, "/v1/rca/Not_Valid").unwrap_err().status, 400);
    assert_eq!(get(&root, "/v1/rca/no-such-rca").unwrap_err().status, 404);
}

#[test]
fn a_root_without_rcas_is_empty_and_never_created() {
    let dir = tempfile::tempdir().unwrap();
    let body = get(dir.path(), "/v1/rca").unwrap();
    assert_eq!(body["rcas"], json!([]));
    assert_eq!(get(dir.path(), "/v1/rca/anything").unwrap_err().status, 404);
    assert!(!dir.path().join("rcas").exists());
}

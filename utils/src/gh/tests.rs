// Tests fail loudly on setup errors; unwrap is the clearest way to do that.
#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn parses_every_documented_state() {
    let parse = |s: &str| parse_pr_state(s);
    assert_eq!(
        parse(r#"{"state":"OPEN","isDraft":false}"#),
        Some(PrState::Open)
    );
    assert_eq!(
        parse(r#"{"state":"OPEN","isDraft":true}"#),
        Some(PrState::Draft)
    );
    assert_eq!(parse(r#"{"state":"MERGED"}"#), Some(PrState::Merged));
    assert_eq!(
        parse(r#"{"state":"CLOSED","isDraft":true,"extra":1}"#),
        Some(PrState::Closed)
    );
}

#[test]
fn rejects_unknown_state_and_garbage() {
    assert_eq!(parse_pr_state(r#"{"state":"WEIRD"}"#), None);
    assert_eq!(parse_pr_state("not json"), None);
    assert_eq!(parse_pr_state(r#"{"isDraft":true}"#), None);
}

#[test]
fn state_order_words_and_serde_agree() {
    let mut states = [
        PrState::Closed,
        PrState::Merged,
        PrState::Open,
        PrState::Draft,
    ];
    states.sort();
    assert_eq!(states, PrState::ALL);
    for state in PrState::ALL {
        assert_eq!(state.to_string(), state.as_str());
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, format!("\"{}\"", state.as_str()));
        assert_eq!(serde_json::from_str::<PrState>(&json).unwrap(), state);
    }
    assert!(PrState::Draft.is_open() && PrState::Open.is_open());
    assert!(!PrState::Merged.is_open() && !PrState::Closed.is_open());
}

#[test]
fn only_etxtbsy_counts_as_text_file_busy() {
    assert_eq!(
        text_file_busy(&io::Error::from_raw_os_error(26)),
        cfg!(unix)
    );
    assert!(!text_file_busy(&io::Error::from(io::ErrorKind::NotFound)));
    assert!(!text_file_busy(&io::Error::from_raw_os_error(2)));
}

#[test]
fn missing_gh_says_how_to_install_it() {
    let missing = Path::new("/nonexistent/beagle-test/gh");
    let err = run_from(missing, &["--version"], Duration::from_secs(5)).unwrap_err();
    assert!(matches!(err, Error::Launch(_)), "{err:?}");
    assert!(
        err.to_string().contains("is the GitHub CLI installed?"),
        "{err}"
    );
}

const TWO_ACCOUNTS_JSON: &str = r#"{"hosts":{"github.com":[
  {"state":"success","active":true,"host":"github.com","login":"octo-work","tokenSource":"keyring"},
  {"state":"success","active":false,"host":"github.com","login":"octo-personal","tokenSource":"keyring"}
],"ghe.example.com":[{"active":true,"host":"ghe.example.com","login":"elsewhere"}]}}"#;

fn account(login: &str, active: bool) -> Account {
    Account {
        login: login.to_owned(),
        active,
    }
}

#[test]
fn status_json_lists_github_com_accounts_in_order() {
    assert_eq!(
        parse_status_json(TWO_ACCOUNTS_JSON).unwrap(),
        vec![account("octo-work", true), account("octo-personal", false)]
    );
    assert_eq!(parse_status_json(r#"{"hosts":{}}"#).unwrap(), vec![]);
    assert_eq!(parse_status_json("unknown flag: --json"), None);
}

#[test]
fn status_text_from_an_older_gh() {
    let text = "github.com
  ✓ Logged in to github.com account octo-work (keyring)
  - Active account: false
  - Token scopes: 'repo'

  ✓ Logged in to github.com account octo-personal (keyring)
  - Active account: true
";
    assert_eq!(
        parse_status_text(text),
        vec![account("octo-work", false), account("octo-personal", true)]
    );
    assert_eq!(
        parse_status_text("You are not logged into any GitHub hosts."),
        vec![]
    );
}

#[cfg(unix)]
mod fake_gh {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Writes an executable `gh` shell script with `body` into `dir`.
    fn fake(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("gh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn a_hung_gh_times_out_with_an_actionable_message() {
        let dir = tempfile::tempdir().unwrap();
        let gh = fake(dir.path(), "exec sleep 5");
        let err = run_from(&gh, &["pr", "view"], Duration::from_millis(200)).unwrap_err();
        assert!(matches!(err, Error::TimedOut(_)), "{err:?}");
        let msg = err.to_string();
        assert!(msg.contains("timed out after 200ms"), "{msg}");
        assert!(msg.ends_with("is GitHub reachable?"), "{msg}");
    }

    #[test]
    fn a_failing_gh_returns_its_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let gh = fake(dir.path(), "echo '  not logged in  ' >&2; exit 4");
        let err = run_from(&gh, &["auth", "token"], Duration::from_secs(5)).unwrap_err();
        assert_eq!(err.to_string(), "not logged in");
    }

    #[test]
    fn pr_state_passes_the_repo_and_parses_the_answer() {
        let dir = tempfile::tempdir().unwrap();
        // Answers with a draft PR only for the expected argv; anything else
        // echoes the argv back, which then fails to parse.
        let gh = fake(
            dir.path(),
            r#"[ "$*" = "pr view 7 --json state,isDraft -R acme/widgets" ] \
               && echo '{"state":"OPEN","isDraft":true}' || echo "$*""#,
        );
        let state = pr_state_from(&gh, "7", Some("acme/widgets"), Duration::from_secs(5)).unwrap();
        assert_eq!(state, PrState::Draft);
        let err = pr_state_from(&gh, "7", None, Duration::from_secs(5)).unwrap_err();
        assert!(matches!(err, Error::Unexpected(_)), "{err:?}");
    }

    #[test]
    fn accounts_reads_gh_auth_status_json_even_on_a_failed_exit() {
        let dir = tempfile::tempdir().unwrap();
        let gh = fake(
            dir.path(),
            &format!(
                "[ \"$*\" = \"auth status --json hosts\" ] && echo '{}'; exit 1",
                TWO_ACCOUNTS_JSON.replace('\n', " ")
            ),
        );
        let got = accounts_from(&gh, Duration::from_secs(5)).unwrap();
        assert_eq!(
            got.iter().map(|a| a.login.as_str()).collect::<Vec<_>>(),
            ["octo-work", "octo-personal"]
        );
    }

    #[test]
    fn accounts_falls_back_to_text_for_an_old_gh() {
        let dir = tempfile::tempdir().unwrap();
        let gh = fake(
            dir.path(),
            r#"case "$*" in
  *--json*) echo 'unknown flag: --json' >&2; exit 1 ;;
  "auth status") printf 'github.com\n  ✓ Logged in to github.com account octo-solo (keyring)\n' >&2 ;;
esac"#,
        );
        assert_eq!(
            accounts_from(&gh, Duration::from_secs(5)).unwrap(),
            vec![account("octo-solo", false)]
        );
        let other = tempfile::tempdir().unwrap();
        let broken = fake(other.path(), "echo 'gh exploded' >&2; exit 2");
        let err = accounts_from(&broken, Duration::from_secs(5)).unwrap_err();
        assert!(err.to_string().contains("gh exploded"), "{err}");
    }

    #[test]
    fn token_for_names_the_account() {
        let dir = tempfile::tempdir().unwrap();
        let gh = fake(
            dir.path(),
            r#"[ "$*" = "auth token -u octo-personal" ] && echo ' tok-personal ' || echo ''"#,
        );
        let got = token_from(&gh, "octo-personal", Duration::from_secs(5)).unwrap();
        assert_eq!(got, "tok-personal");
        let err = token_from(&gh, "octo-work", Duration::from_secs(5)).unwrap_err();
        assert!(matches!(err, Error::Unexpected(_)), "{err:?}");
    }
}

//! Account/repo discovery + collaborator lookups (repo browser and
//! @people autocomplete).

use tandem_core::github::RepoRef;
use tandem_core::Result;

use crate::client::GithubClient;

/// One commit on a pull request, for the PR-header commits list.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PrCommit {
    /// Full commit SHA.
    pub sha: String,
    /// Full commit message (subject and body).
    pub message: String,
    /// GitHub login when the commit is linked to an account, else the git
    /// author name, else `"unknown"`.
    pub author: String,
    /// Git author date as GitHub reports it (ISO 8601), if present.
    pub authored_at: Option<String>,
}
use crate::wire::{MergeOptions, RepoSummary};

impl GithubClient {
    /// Collaborator logins for a repo (used for @people autocomplete).
    /// Listing needs push access on some repos — callers treat failure
    /// as "no extra names", not an error.
    pub async fn list_collaborators(&self, repo: &RepoRef) -> Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct Collaborator {
            login: String,
        }
        let people: Vec<Collaborator> = self
            .get_json(&format!(
                "/repos/{}/{}/collaborators?per_page=100",
                repo.owner, repo.name
            ))
            .await?;
        Ok(people.into_iter().map(|c| c.login).collect())
    }

    /// Login of the authenticated user (used to seed the repo browser).
    pub async fn viewer_login(&self) -> Result<String> {
        #[derive(serde::Deserialize)]
        struct Viewer {
            login: String,
        }
        let v: Viewer = self.get_json("/user").await?;
        Ok(v.login)
    }

    /// Organizations the authenticated user belongs to.
    pub async fn list_orgs(&self) -> Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct Org {
            login: String,
        }
        let orgs: Vec<Org> = self.get_json("/user/orgs?per_page=100").await?;
        Ok(orgs.into_iter().map(|o| o.login).collect())
    }

    /// Repos for an owner, most recently pushed first. `viewer` selects
    /// the authenticated-user endpoint (which includes private repos).
    pub async fn list_owner_repos(&self, owner: &str, viewer: bool) -> Result<Vec<RepoSummary>> {
        let path = if viewer {
            "/user/repos?per_page=100&sort=pushed&affiliation=owner".to_owned()
        } else {
            format!("/orgs/{owner}/repos?per_page=100&sort=pushed")
        };
        self.get_json(&path).await
    }

    /// Which merge methods (and auto-merge) the repo allows. Fields are
    /// omitted for viewers without push access — default to permissive
    /// for methods and OFF for auto-merge.
    pub async fn repo_merge_options(&self, repo: &RepoRef) -> Result<MergeOptions> {
        #[derive(serde::Deserialize)]
        struct RepoSettings {
            #[serde(default, rename = "allow_squash_merge")]
            squash: Option<bool>,
            #[serde(default, rename = "allow_merge_commit")]
            merge: Option<bool>,
            #[serde(default, rename = "allow_rebase_merge")]
            rebase: Option<bool>,
            #[serde(default, rename = "allow_auto_merge")]
            auto_merge: Option<bool>,
        }
        let settings: RepoSettings = self
            .get_json(&format!("/repos/{}/{}", repo.owner, repo.name))
            .await?;
        Ok(MergeOptions {
            squash: settings.squash.unwrap_or(true),
            merge: settings.merge.unwrap_or(true),
            rebase: settings.rebase.unwrap_or(true),
            auto_merge: settings.auto_merge.unwrap_or(false),
        })
    }

    /// Commits on the PR, in GitHub's (oldest-first) order.
    pub async fn pr_commits(&self, repo: &RepoRef, number: u64) -> Result<Vec<PrCommit>> {
        #[derive(serde::Deserialize)]
        struct Raw {
            sha: String,
            commit: RawCommit,
            author: Option<RawUser>,
        }
        #[derive(serde::Deserialize)]
        struct RawCommit {
            message: String,
            author: Option<RawMeta>,
        }
        #[derive(serde::Deserialize)]
        struct RawMeta {
            name: Option<String>,
            date: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct RawUser {
            login: String,
        }
        let raw: Vec<Raw> = self
            .get_json(&format!(
                "/repos/{}/{}/pulls/{number}/commits?per_page=100",
                repo.owner, repo.name
            ))
            .await?;
        Ok(raw
            .into_iter()
            .map(|c| PrCommit {
                sha: c.sha,
                message: c.commit.message,
                author: c
                    .author
                    .map(|a| a.login)
                    .or(c.commit.author.as_ref().and_then(|a| a.name.clone()))
                    .unwrap_or_else(|| "unknown".into()),
                authored_at: c.commit.author.and_then(|a| a.date),
            })
            .collect())
    }

    /// Filenames changed by the PR (first 100 — enough to spot conflict
    /// candidates; huge PRs are truncated).
    pub async fn pr_files(&self, repo: &RepoRef, number: u64) -> Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct PrFile {
            filename: String,
        }
        let files: Vec<PrFile> = self
            .get_json(&format!(
                "/repos/{}/{}/pulls/{number}/files?per_page=100",
                repo.owner, repo.name
            ))
            .await?;
        Ok(files.into_iter().map(|f| f.filename).collect())
    }

    /// Files changed on the base branch since the merge-base with
    /// `head_sha`, with their patches — the other side of a conflict.
    pub async fn base_changes_since(
        &self,
        repo: &RepoRef,
        head_sha: &str,
        base_ref: &str,
    ) -> Result<Vec<(String, Option<String>)>> {
        #[derive(serde::Deserialize)]
        struct CompareFile {
            filename: String,
            patch: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct Compare {
            #[serde(default)]
            files: Vec<CompareFile>,
        }
        let cmp: Compare = self
            .get_json(&format!(
                "/repos/{}/{}/compare/{head_sha}...{base_ref}",
                repo.owner, repo.name
            ))
            .await?;
        Ok(cmp
            .files
            .into_iter()
            .map(|f| (f.filename, f.patch))
            .collect())
    }
}

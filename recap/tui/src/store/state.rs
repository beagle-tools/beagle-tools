//! Recording a PR's live state in its manifest, and archiving merged PRs.

use super::files::{read_toml, write_atomic};
use super::{Store, RECAP_TOML};
use crate::error::{Error, Result};
use crate::model::{PrState, RecapId, RecapManifest};

impl Store {
    /// Records a PR's live state (and when it last changed) in its
    /// `recap.toml`. Returns `true` when the file changed. Never touches the
    /// summary, diff or diagrams.
    ///
    /// # Errors
    /// [`Error::NoSuchRecap`], [`Error::ParseToml`] or [`Error::Io`].
    pub fn set_state(&self, id: &RecapId, state: PrState, updated: Option<&str>) -> Result<bool> {
        let path = self.recap_dir(id).join(RECAP_TOML);
        let mut manifest =
            read_toml::<RecapManifest>(&path)?.ok_or_else(|| Error::NoSuchRecap(id.to_string()))?;
        let updated = updated
            .map(str::to_owned)
            .or_else(|| manifest.updated.clone());
        if manifest.state == state && manifest.updated == updated {
            return Ok(false);
        }
        manifest.state = state;
        manifest.updated = updated;
        write_atomic(&path, &toml::to_string(&manifest)?)?;
        Ok(true)
    }

    /// Records when a PR merged, if `recap.toml` doesn't have it yet.
    /// Returns `true` when the file changed.
    ///
    /// # Errors
    /// [`Error::NoSuchRecap`], [`Error::ParseToml`] or [`Error::Io`].
    pub fn set_merged_at(&self, id: &RecapId, merged_at: &str) -> Result<bool> {
        let path = self.recap_dir(id).join(RECAP_TOML);
        let mut manifest =
            read_toml::<RecapManifest>(&path)?.ok_or_else(|| Error::NoSuchRecap(id.to_string()))?;
        if manifest.merged_at.as_deref() == Some(merged_at) {
            return Ok(false);
        }
        manifest.merged_at = Some(merged_at.to_owned());
        write_atomic(&path, &toml::to_string(&manifest)?)?;
        Ok(true)
    }

    /// Archives every merged recap whose PR merged at least `max_age_secs`
    /// before `now` (Unix seconds). The merge time is `merged_at`, or, for
    /// older recaps without it, `updated`. Recaps with neither stay put.
    /// Returns the ids archived. Nothing is deleted — the archive keeps them.
    ///
    /// # Errors
    /// [`Error::Io`] if the store can't be listed or a move fails.
    pub fn archive_merged_older_than(&self, now: i64, max_age_secs: i64) -> Result<Vec<String>> {
        let (rows, _) = self.list()?;
        let mut archived = Vec::new();
        for row in rows.iter().filter(|r| r.state == PrState::Merged) {
            let Ok(id) = RecapId::new(&row.id) else {
                continue;
            };
            let Ok(Some(m)) = read_toml::<RecapManifest>(&self.recap_dir(&id).join(RECAP_TOML))
            else {
                continue;
            };
            let merged = m
                .merged_at
                .as_deref()
                .or(m.updated.as_deref())
                .and_then(crate::model::unix_seconds);
            if merged.is_some_and(|t| now - t >= max_age_secs) {
                self.archive(&id)?;
                archived.push(row.id.clone());
            }
        }
        Ok(archived)
    }
}

/// How long a merged recap stays on the active list before it's archived.
pub const AUTO_ARCHIVE_AFTER_SECS: i64 = 24 * 60 * 60;

/// Now, in Unix seconds.
#[must_use]
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

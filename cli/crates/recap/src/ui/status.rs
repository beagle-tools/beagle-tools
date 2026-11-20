//! Live PR status from the background poller, and the 24h auto-archive of
//! merged PRs.

use std::collections::HashMap;

use super::App;
use crate::model::RecapId;
use crate::poller::PrStatus;
use crate::store::Shelf;

impl App {
    /// Every PR URL in the list, for the poller.
    pub(super) fn pr_urls(&self) -> Vec<String> {
        // The archive isn't polled: its PRs are done with.
        if self.shelf == Shelf::Archived {
            return Vec::new();
        }
        self.rows.iter().map(|r| r.url.clone()).collect()
    }

    /// The live status of the selected PR, if the poller has reported it.
    pub(super) fn selected_status(&self) -> Option<&PrStatus> {
        self.selected_row().and_then(|r| self.statuses.get(&r.url))
    }

    /// Takes a batch from the poller: remembers each PR's live status, and
    /// when a PR's state moved (draft → open, open → merged, …) records it
    /// in that recap's `recap.toml` and re-sorts the list.
    pub fn apply_statuses(&mut self, batch: HashMap<String, PrStatus>) {
        let mut moved = Vec::new();
        for row in &self.rows {
            let Some(status) = batch.get(&row.url) else {
                continue;
            };
            if status.state == row.state {
                continue;
            }
            let Ok(id) = RecapId::new(&row.id) else {
                continue;
            };
            match self
                .store
                .set_state(&id, status.state, status.updated.as_deref())
            {
                Ok(_) => moved.push(format!(
                    "{}#{} {} → {}",
                    row.repo.rsplit('/').next().unwrap_or(&row.repo),
                    row.number,
                    row.state,
                    status.state
                )),
                Err(e) => self.status = Some(e.to_string()),
            }
        }
        // Remember when merged PRs merged: it drives the 24h auto-archive.
        for row in &self.rows {
            let merged_at = batch.get(&row.url).and_then(|s| s.merged_at.as_deref());
            if let (Some(at), Ok(id)) = (merged_at, RecapId::new(&row.id)) {
                let _ = self.store.set_merged_at(&id, at); // best effort; the next poll retries
            }
        }
        self.statuses.extend(batch);
        if !moved.is_empty() {
            self.reload();
            self.status = Some(format!("PR status: {}", moved.join(" · ")));
        }
        self.auto_archive();
    }

    /// Moves merged recaps whose PR merged over 24h ago to the archive (they
    /// stay readable there: `s`).
    pub(crate) fn auto_archive(&mut self) {
        match self.store.archive_merged_older_than(
            crate::store::now_unix(),
            crate::store::AUTO_ARCHIVE_AFTER_SECS,
        ) {
            Ok(ids) if ids.is_empty() => {}
            Ok(ids) => {
                self.reload();
                self.status = Some(format!(
                    "archived {} (merged over 24h ago — s shows the archive)",
                    ids.join(", ")
                ));
            }
            Err(e) => self.status = Some(format!("auto-archive: {e}")),
        }
    }
}

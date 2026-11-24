//! Edit history for local comments. An agent's comment is a draft you
//! take over: you reword it, then post it as yours. Keeping what it
//! used to say means that hand-off is reversible, and that you can see
//! how much of the posted text was actually the agent's.

use chrono::Utc;
use rusqlite::params;
use tandem_core::review::CommentRevision;
use tandem_core::Result;
use uuid::Uuid;

use crate::schema::cache_err;
use crate::Cache;

/// Edit history of local comment bodies.
pub trait CommentRevisions {
    /// Snapshot the body a comment is about to stop having.
    fn record_revision(&self, comment_id: &str, editor: &str, body: &str) -> Result<()>;
    /// Superseded versions, oldest first. Two edits can land in the
    /// same instant, so insertion order breaks the tie.
    fn list_revisions(&self, comment_id: &str) -> Result<Vec<CommentRevision>>;
    /// How many times this comment has been rewritten.
    fn revision_count(&self, comment_id: &str) -> Result<u32>;
}

impl CommentRevisions for Cache {
    fn record_revision(&self, comment_id: &str, editor: &str, body: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO comment_revisions (id, comment_id, edited_at, editor, body)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    comment_id,
                    Utc::now().to_rfc3339(),
                    editor,
                    body
                ],
            )
            .map_err(cache_err)?;
        Ok(())
    }

    fn list_revisions(&self, comment_id: &str) -> Result<Vec<CommentRevision>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, comment_id, edited_at, editor, body FROM comment_revisions
                 WHERE comment_id = ?1 ORDER BY edited_at ASC, rowid ASC",
            )
            .map_err(cache_err)?;
        let rows = stmt
            .query_map(params![comment_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(cache_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (id, comment_id, edited_at, editor, body) = row.map_err(cache_err)?;
            out.push(CommentRevision {
                id,
                comment_id,
                edited_at: crate::archive::parse_ts(&edited_at)?,
                editor,
                body,
            });
        }
        Ok(out)
    }

    fn revision_count(&self, comment_id: &str) -> Result<u32> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM comment_revisions WHERE comment_id = ?1",
                params![comment_id],
                |row| row.get::<_, u32>(0),
            )
            .map_err(cache_err)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn keeps_versions_oldest_first() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .record_revision("c1", "claude", "first draft")
            .unwrap();
        cache.record_revision("c1", "you", "my rewrite").unwrap();
        cache.record_revision("c2", "you", "elsewhere").unwrap();

        let got = cache.list_revisions("c1").unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].body, "first draft");
        assert_eq!(got[0].editor, "claude");
        assert_eq!(got[1].body, "my rewrite");
        assert_eq!(cache.revision_count("c1").unwrap(), 2);
        assert_eq!(cache.revision_count("never-edited").unwrap(), 0);
    }
}

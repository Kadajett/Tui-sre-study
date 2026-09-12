use crate::{teaching::Teacher, Store};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rusqlite::OptionalExtension;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct ReviewSession {
    pub id: i64,
    pub started_at: String,
}

impl ReviewSession {
    fn active_at(&self, now: DateTime<Utc>) -> Result<bool> {
        let started = DateTime::parse_from_rfc3339(&self.started_at)?;
        Ok(now.signed_duration_since(started) < Duration::hours(24))
    }
}

impl Store {
    fn latest_review_session(&self) -> Result<Option<ReviewSession>> {
        Ok(self
            .db
            .query_row(
                "SELECT id,started_at FROM reinforcement_sessions ORDER BY id DESC LIMIT 1",
                [],
                |row| {
                    Ok(ReviewSession {
                        id: row.get(0)?,
                        started_at: row.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn review_session_at(&self, now: DateTime<Utc>) -> Result<ReviewSession> {
        let tx = self.db.unchecked_transaction()?;
        if let Some(session) = self.latest_review_session()? {
            if session.active_at(now)? {
                tx.commit()?;
                return Ok(session);
            }
        }
        let started_at = now.to_rfc3339();
        self.db.execute(
            "INSERT INTO reinforcement_sessions(started_at) VALUES(?1)",
            [&started_at],
        )?;
        let session = ReviewSession {
            id: self.db.last_insert_rowid(),
            started_at,
        };
        tx.commit()?;
        Ok(session)
    }
}

impl Teacher {
    pub(super) fn touch_review_session(&mut self, now: DateTime<Utc>) -> Result<()> {
        let active = self.progress.learned_at.is_some() || !self.store.teaching_due()?.is_empty();
        self.review_session = if active {
            Some(self.store.review_session_at(now)?)
        } else {
            None
        };
        Ok(())
    }
}

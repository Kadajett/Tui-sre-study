use anyhow::{ensure, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

use crate::{
    du_course::{LAST_STEP, LESSON_ID},
    Store,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub step: usize,
    pub ready: bool,
    pub assisted: bool,
    pub completed_at: Option<String>,
}

impl Store {
    pub fn course_progress(&self) -> Result<Progress> {
        self.db.execute_batch(
            "CREATE TABLE IF NOT EXISTS guided_courses(
            course_id TEXT PRIMARY KEY, step INTEGER NOT NULL,
            ready INTEGER NOT NULL, assisted INTEGER NOT NULL, completed_at TEXT)",
        )?;
        let state = self
            .db
            .query_row(
                "SELECT step,ready,assisted,completed_at FROM guided_courses WHERE course_id=?1",
                [LESSON_ID],
                |row| {
                    Ok(Progress {
                        step: row.get(0)?,
                        ready: row.get(1)?,
                        assisted: row.get(2)?,
                        completed_at: row.get(3)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_default();
        ensure!(
            state.step <= LAST_STEP,
            "Stored du lesson step is not supported by this version"
        );
        Ok(state)
    }

    pub fn save_course(&self, state: &Progress) -> Result<()> {
        self.db.execute(
            "INSERT INTO guided_courses VALUES(?1,?2,?3,?4,?5)
            ON CONFLICT(course_id) DO UPDATE SET step=excluded.step,ready=excluded.ready,
            assisted=excluded.assisted,completed_at=excluded.completed_at",
            params![
                LESSON_ID,
                state.step,
                state.ready,
                state.assisted,
                state.completed_at
            ],
        )?;
        Ok(())
    }

    pub fn graduate(&self, state: &Progress, answer: &str) -> Result<Progress> {
        ensure!(
            state.step == LAST_STEP && state.ready,
            "Finish the challenge before enrolling in reviews"
        );
        let tx = self.db.unchecked_transaction()?;
        let mut completed = state.clone();
        completed.completed_at = Some(Utc::now().to_rfc3339());
        self.review(LESSON_ID, if state.assisted { 3 } else { 4 }, answer)?;
        let (repetitions, interval, delay) = if state.assisted {
            (0, 0, chrono::Duration::minutes(10))
        } else {
            (1, 1, chrono::Duration::days(1))
        };
        let due = (Utc::now() + delay).to_rfc3339();
        self.db.execute(
            "UPDATE reviews SET due_at=?1,repetitions=?2,interval_days=?3 WHERE lesson_id=?4",
            params![due, repetitions, interval, LESSON_ID],
        )?;
        self.save_course(&completed)?;
        tx.commit()?;
        Ok(completed)
    }

    pub fn next_du_review(&self) -> Result<Option<String>> {
        Ok(self
            .db
            .query_row(
                "SELECT due_at FROM reviews WHERE lesson_id=?1",
                [LESSON_ID],
                |row| row.get(0),
            )
            .optional()?)
    }
}

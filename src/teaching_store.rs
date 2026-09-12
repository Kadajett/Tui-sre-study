use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::Store;

#[derive(Clone, Debug, Default)]
pub struct TopicProgress {
    pub step: usize,
    pub ready: bool,
    pub practice_offered: bool,
    pub learned_at: Option<String>,
    pub pending_review: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct DueTopic {
    pub topic_id: String,
    pub due_at: String,
    pub last_answer: String,
    pub notes: String,
    pub last_score: Option<i32>,
}

impl Store {
    pub fn prepare_teaching(&self) -> Result<()> {
        self.db.execute_batch("CREATE TABLE IF NOT EXISTS teaching_topics(
            topic_id TEXT PRIMARY KEY, step INTEGER NOT NULL DEFAULT 0,
            ready INTEGER NOT NULL DEFAULT 0, practice_offered INTEGER NOT NULL DEFAULT 0,
            learned_at TEXT, pending_review TEXT, notes TEXT NOT NULL DEFAULT '');
            CREATE TABLE IF NOT EXISTS teaching_position(scope TEXT PRIMARY KEY, topic_id TEXT NOT NULL);")?;
        let old = self.course_progress()?;
        if old.completed_at.is_some() || old.step > 0 || old.ready {
            self.db.execute("INSERT OR IGNORE INTO teaching_topics(topic_id,step,ready,learned_at,notes) VALUES('linux-du-1',?1,?2,?3,'Progress carried from the previous guided du course')", params![old.step.min(4),old.ready || old.step>4,old.completed_at])?;
        }
        Ok(())
    }

    pub fn topic_progress(&self, id: &str) -> Result<TopicProgress> {
        Ok(self.db.query_row("SELECT step,ready,practice_offered,learned_at,pending_review FROM teaching_topics WHERE topic_id=?1", [id], |row| Ok(TopicProgress {
            step: row.get(0)?, ready: row.get(1)?, practice_offered: row.get(2)?, learned_at: row.get(3)?, pending_review: row.get(4)?,
        })).optional()?.unwrap_or_default())
    }

    pub fn save_topic(&self, id: &str, state: &TopicProgress) -> Result<()> {
        self.db.execute("INSERT INTO teaching_topics(topic_id,step,ready,practice_offered,learned_at,pending_review) VALUES(?1,?2,?3,?4,?5,?6)
            ON CONFLICT(topic_id) DO UPDATE SET step=excluded.step,ready=excluded.ready,practice_offered=excluded.practice_offered,learned_at=excluded.learned_at,pending_review=excluded.pending_review",
            params![id,state.step,state.ready,state.practice_offered,state.learned_at,state.pending_review])?;
        Ok(())
    }

    pub fn teaching_position(&self, scope: &str) -> Result<Option<String>> {
        Ok(self
            .db
            .query_row(
                "SELECT topic_id FROM teaching_position WHERE scope=?1",
                [scope],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_teaching_position(&self, scope: &str, id: &str) -> Result<()> {
        self.db.execute("INSERT INTO teaching_position VALUES(?1,?2) ON CONFLICT(scope) DO UPDATE SET topic_id=excluded.topic_id", params![scope,id])?;
        Ok(())
    }

    pub fn learn_topic(
        &self,
        id: &str,
        state: &TopicProgress,
        evidence: &str,
    ) -> Result<TopicProgress> {
        let tx = self.db.unchecked_transaction()?;
        let learned = self.enroll_topic(id, state, evidence)?;
        tx.commit()?;
        Ok(learned)
    }

    pub fn enroll_topic(
        &self,
        id: &str,
        state: &TopicProgress,
        evidence: &str,
    ) -> Result<TopicProgress> {
        let mut learned = state.clone();
        learned.ready = true;
        if learned.learned_at.is_some() {
            return Ok(learned);
        }
        learned.learned_at = Some(Utc::now().to_rfc3339());
        self.save_topic(id, &learned)?;
        // Legacy quiz attempts are kept, but they don't count as introductory teaching.
        self.db.execute("INSERT INTO reviews(lesson_id,repetitions,interval_days,ease,due_at,attempts,correct) VALUES(?1,0,0,2.5,?2,0,0)
            ON CONFLICT(lesson_id) DO UPDATE SET due_at=excluded.due_at,repetitions=0,interval_days=0", params![id,(Utc::now()+chrono::Duration::days(1)).to_rfc3339()])?;
        self.db.execute(
            "UPDATE teaching_topics SET notes=?1 WHERE topic_id=?2",
            params![evidence, id],
        )?;
        Ok(learned)
    }

    pub fn teaching_due(&self) -> Result<Vec<DueTopic>> {
        let mut statement = self.db.prepare("SELECT r.lesson_id,r.due_at,COALESCE(r.last_answer,''),t.notes,r.last_score FROM reviews r JOIN teaching_topics t ON t.topic_id=r.lesson_id WHERE t.learned_at IS NOT NULL AND julianday(r.due_at)<=julianday(?1) ORDER BY julianday(r.due_at) LIMIT 3")?;
        let rows = statement.query_map([Utc::now().to_rfc3339()], |row| {
            Ok(DueTopic {
                topic_id: row.get(0)?,
                due_at: row.get(1)?,
                last_answer: row.get(2)?,
                notes: row.get(3)?,
                last_score: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn teaching_history(&self, id: &str) -> Result<Vec<Value>> {
        let mut statement = self.db.prepare(
            "SELECT role,content FROM tutor_memory WHERE lesson_id=?1 ORDER BY id DESC LIMIT 12",
        )?;
        let rows = statement.query_map([format!("teacher:{id}")], |row| {
            Ok(json!({"role":row.get::<_,String>(0)?,"content":row.get::<_,String>(1)?}))
        })?;
        let mut messages = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        messages.reverse();
        Ok(messages)
    }

    pub fn save_teaching_exchange(
        &self,
        id: &str,
        learner: Option<&str>,
        teacher: &str,
    ) -> Result<()> {
        if let Some(learner) = learner {
            self.save_teaching_message(id, "user", learner)?;
        }
        self.save_teaching_message(id, "assistant", teacher)
    }

    pub fn save_teaching_message(&self, id: &str, role: &str, content: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO tutor_memory(lesson_id,role,content,created_at) VALUES(?1,?2,?3,?4)",
            params![
                format!("teacher:{id}"),
                role,
                content,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }
}

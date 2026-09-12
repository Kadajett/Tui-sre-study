use crate::Store;
use anyhow::Result;
use rusqlite::{params, OptionalExtension};

impl Store {
    pub fn prepare_chat(&self) -> Result<()> {
        self.db.execute_batch("CREATE TABLE IF NOT EXISTS teaching_chat (
            id INTEGER PRIMARY KEY, scope TEXT NOT NULL, body TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS teaching_chat_scope ON teaching_chat(scope,id);
            CREATE TABLE IF NOT EXISTS teaching_resume (scope TEXT PRIMARY KEY, snapshot TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS reinforcement_sessions (id INTEGER PRIMARY KEY, started_at TEXT NOT NULL);")?;
        Ok(())
    }

    pub fn chat_resume(&self, scope: &str) -> Result<Option<String>> {
        Ok(self
            .db
            .query_row(
                "SELECT snapshot FROM teaching_resume WHERE scope=?1",
                [scope],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn chat_messages(&self, scope: &str) -> Result<Vec<String>> {
        let mut statement = self
            .db
            .prepare("SELECT body FROM teaching_chat WHERE scope=?1 ORDER BY id")?;
        let rows = statement.query_map([scope], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // The caller owns the transaction so a reply and its learning assessment commit together.
    pub fn write_chat(&self, scope: &str, snapshot: &str, messages: &[String]) -> Result<()> {
        for message in messages {
            self.db.execute(
                "INSERT INTO teaching_chat(scope,body) VALUES(?1,?2)",
                params![scope, message],
            )?;
        }
        self.db.execute("INSERT INTO teaching_resume VALUES(?1,?2) ON CONFLICT(scope) DO UPDATE SET snapshot=excluded.snapshot", params![scope,snapshot])?;
        Ok(())
    }

    pub fn legacy_chat(&self, lessons: &[crate::Lesson]) -> Result<Vec<String>> {
        let mut statement = self.db.prepare("SELECT lesson_id,role,content FROM tutor_memory WHERE lesson_id LIKE 'teacher:%' ORDER BY id")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut messages = Vec::new();
        for row in rows {
            let (id, role, content) = row?;
            if !lessons
                .iter()
                .any(|lesson| id == format!("teacher:{}", lesson.id))
            {
                continue;
            }
            let speaker = if role == "user" { "You" } else { "Mercury" };
            messages.push(format!("{speaker}\n{content}"));
        }
        Ok(messages)
    }
}

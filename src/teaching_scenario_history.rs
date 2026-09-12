use crate::{
    teaching::{Teacher, Turn, TurnKind},
    teaching_protocol::{Assessment, TeachingReply},
};
use anyhow::Result;

impl Teacher {
    pub(super) fn scenario_history_key(&self) -> String {
        format!(
            "scenario:{}:{}",
            self.lab.active.as_deref().unwrap_or("none"),
            self.lab.started_at
        )
    }

    pub(super) fn apply_scenario_reply(
        &mut self,
        reply: &TeachingReply,
        turn: &Turn,
    ) -> Result<bool> {
        let TurnKind::Scenario {
            ref id,
            evaluate,
            verified,
        } = turn.kind
        else {
            return Ok(false);
        };
        if self.lab.active.as_ref() != Some(id) {
            return Ok(true);
        }
        self.store.save_teaching_exchange(
            &self.scenario_history_key(),
            Some(&turn.text),
            &reply.message,
        )?;
        self.say("Mercury", &reply.message);
        if evaluate
            && verified
            && reply.assessment == Assessment::Understood
            && !reply.evidence.trim().is_empty()
        {
            self.lab.completed = true;
            self.say("Incident resolved", "Your real recovery check and explanation passed. The evidence remains saved. /lab stop cleans up; your normal lesson is still saved.");
        }
        self.archive_scenario()?;
        Ok(true)
    }

    pub(super) fn archive_scenario(&self) -> Result<()> {
        if !self.lab.scenario {
            return Ok(());
        }
        self.store.db.execute_batch("CREATE TABLE IF NOT EXISTS scenario_attempts(attempt_id TEXT PRIMARY KEY, scope TEXT NOT NULL, session_id INTEGER, state TEXT NOT NULL)")?;
        self.store.db.execute("INSERT INTO scenario_attempts VALUES(?1,?2,?3,?4) ON CONFLICT(attempt_id) DO UPDATE SET state=excluded.state",
            rusqlite::params![self.scenario_history_key(),self.scope,self.review_session.as_ref().map(|session|session.id),serde_json::to_string(&self.lab)?])?;
        Ok(())
    }
}

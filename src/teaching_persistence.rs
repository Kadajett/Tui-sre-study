use crate::{
    teaching::{Teacher, Turn, TurnKind},
    teaching_curriculum as curriculum,
    teaching_view::{Pane, Viewport},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Default, Serialize, Deserialize)]
struct ReadingPosition {
    top: usize,
    follow: bool,
}

impl ReadingPosition {
    fn capture(view: &Viewport) -> Self {
        Self {
            top: view.top,
            follow: view.follow,
        }
    }
    fn restore(&self, view: &mut Viewport) {
        view.top = self.top;
        view.follow = self.follow;
    }
}

#[derive(Serialize, Deserialize)]
struct Resume {
    version: u8,
    topic: String,
    input: String,
    output: String,
    generation: usize,
    queue: VecDeque<Turn>,
    in_flight: Option<Turn>,
    conversation: ReadingPosition,
    console: ReadingPosition,
    output_focused: bool,
}

impl Resume {
    fn capture(app: &Teacher) -> Self {
        Self {
            version: 1,
            topic: app.lesson().id.clone(),
            input: app.input.clone(),
            output: app.output.clone(),
            generation: app.generation,
            queue: app.queue.clone(),
            in_flight: app.in_flight.clone(),
            conversation: ReadingPosition::capture(&app.view.conversation),
            console: ReadingPosition::capture(&app.view.output),
            output_focused: app.view.focused == Pane::Output,
        }
    }
}

impl Teacher {
    pub(super) fn restore_or_start(&mut self) -> Result<()> {
        if !self.restore_chat()? {
            self.open_topic()?;
        }
        self.touch_review_session(chrono::Utc::now())?;
        self.checkpoint()
    }

    pub(super) fn restore_chat(&mut self) -> Result<bool> {
        let Some(json) = self.store.chat_resume(&self.scope)? else {
            self.transcript = self.store.legacy_chat(&self.lessons)?;
            return Ok(!self.transcript.is_empty());
        };
        let resume: Resume = serde_json::from_str(&json)?;
        ensure!(
            resume.version == 1,
            "Unsupported saved conversation version"
        );
        self.index = self
            .lessons
            .iter()
            .position(|lesson| lesson.id == resume.topic)
            .ok_or_else(|| {
                anyhow::anyhow!("Saved conversation topic is missing from this collection")
            })?;
        self.progress = self.store.topic_progress(&resume.topic)?;
        ensure!(
            self.progress.step < curriculum::step_count(self.lesson()),
            "Unsupported saved lesson step"
        );
        self.transcript = self.store.chat_messages(&self.scope)?;
        self.saved_messages = self.transcript.len();
        self.input = resume.input;
        self.output = resume.output;
        self.generation = resume.generation;
        self.queue = resume.queue;
        // Retry the model turn with its saved evidence; never re-execute its command.
        if let Some(turn) = resume.in_flight {
            self.queue.push_front(turn);
        }
        resume.conversation.restore(&mut self.view.conversation);
        resume.console.restore(&mut self.view.output);
        self.view.focused = if resume.output_focused {
            Pane::Output
        } else {
            Pane::Conversation
        };
        Ok(true)
    }

    pub fn checkpoint(&mut self) -> Result<()> {
        let json = serde_json::to_string(&Resume::capture(self))?;
        if json == self.last_checkpoint && self.saved_messages == self.transcript.len() {
            return Ok(());
        }
        let tx = self.store.db.unchecked_transaction()?;
        self.store
            .write_chat(&self.scope, &json, &self.transcript[self.saved_messages..])?;
        tx.commit()?;
        self.saved_messages = self.transcript.len();
        self.last_checkpoint = json;
        Ok(())
    }

    pub(super) fn checkpoint_reply(&self, extra: &[String]) -> Result<String> {
        let json = serde_json::to_string(&Resume::capture(self))?;
        let mut messages = self.transcript[self.saved_messages..].to_vec();
        messages.extend_from_slice(extra);
        self.store.write_chat(&self.scope, &json, &messages)?;
        Ok(json)
    }

    pub(super) fn open_topic(&mut self) -> Result<()> {
        self.store
            .set_teaching_position(&self.scope, &self.lesson().id)?;
        self.say(
            "Let's learn",
            &curriculum::introduction(self.lesson(), self.progress.step),
        );
        if self.progress.learned_at.is_some() {
            self.say("Coach", "We've already worked through this topic. We can revisit it conversationally, or /next introduces the next topic.");
        }
        self.queue.push_back(Turn { text: "Introduce this new teaching step with a concrete worked example. Do not ask a quiz or review question on this opening turn.".into(), kind: TurnKind::Introduction, can_assess: false, review_for: None, output: String::new(), lab_topic: None });
        Ok(())
    }
}

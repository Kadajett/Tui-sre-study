use crate::{
    teaching::{Teacher, Turn, TurnKind},
    teaching_curriculum as curriculum,
    teaching_protocol::{Assessment, Outcome, TeachingReply},
    teaching_store::DueTopic,
};
use anyhow::Result;
use serde_json::json;

impl Teacher {
    pub fn tick(&mut self) -> Result<()> {
        self.poll_lab()?;
        if !self.online {
            return Ok(());
        }
        self.receive_pending()?;
        if self.coach.busy() {
            return Ok(());
        }
        let Some(turn) = self.queue.pop_front() else {
            return Ok(());
        };
        let prompt = self.context(&turn)?;
        let history = self.history_for(&turn)?;
        self.in_flight = Some(turn.clone());
        if let Err(error) = self.checkpoint() {
            self.in_flight = None;
            self.queue.push_front(turn);
            return Err(error);
        }
        self.coach.teach(self.generation, prompt, history);
        Ok(())
    }

    pub(super) fn history_for(&self, turn: &Turn) -> Result<Vec<serde_json::Value>> {
        // An explicit new-step introduction uses its authored goal, not prior model suggestions.
        // The full chat remains saved and ordinary questions retain their conversation history.
        if matches!(turn.kind, TurnKind::Introduction) {
            return Ok(Vec::new());
        }
        let key = if matches!(turn.kind, TurnKind::Scenario { .. }) {
            self.scenario_history_key()
        } else {
            self.lesson().id.clone()
        };
        self.store.teaching_history(&key)
    }

    fn receive_pending(&mut self) -> Result<()> {
        if let Some(reply) = self.coach.poll() {
            let turn = self.in_flight.take();
            if reply.generation == self.generation {
                if let Some(turn) = turn {
                    self.receive(reply.text, &turn)?;
                    self.checkpoint()?;
                }
            }
        }
        Ok(())
    }

    pub fn context(&self, turn: &Turn) -> Result<String> {
        let due = self
            .store
            .teaching_due()?
            .into_iter()
            .filter_map(|review| {
                self.catalog.iter().find(|lesson| lesson.id == review.topic_id).map(|lesson| json!({
                "history":review,"title":curriculum::title(lesson),"reference":lesson.answer,
            }))
            })
            .collect::<Vec<_>>();
        let intent = match turn.kind {
            TurnKind::Introduction => "explain_first_no_questions",
            TurnKind::Practice => "guide_a_small_example_or_explain_more",
            TurnKind::Conversation => "respond_to_learner",
            TurnKind::Lab(_) => "explain_actual_command_output",
            TurnKind::Scenario { .. } => "coach_or_assess_live_incident",
        };
        Ok(json!({
            "scenario":match &turn.kind {TurnKind::Scenario {id,evaluate,verified}=>Some(json!({"briefing":crate::scenario_catalog::catalog().into_iter().find(|s| &s.id==id),"evaluate":evaluate,"recovery_verified":verified,"supported_commands":crate::lab_commands::available(id)})),_=>None},
            "current_topic":{"id":self.lesson().id,"title":curriculum::title(self.lesson()),"step_title":curriculum::step_title(self.lesson(), self.progress.step),"supported_commands":curriculum::commands(self.lesson(), self.progress.step),"teaching_material":curriculum::introduction(self.lesson(),self.progress.step),"practice_goal":curriculum::practice_goal(self.lesson(), self.progress.step),"step":self.progress.step+1,"step_count":curriculum::step_count(self.lesson()),"practice_mode":self.lesson().kind,"step_practiced":self.progress.ready,"already_learned":self.progress.learned_at.is_some()},
            "navigation_policy":"The saved current_topic is authoritative. Earlier assistant suggestions do not advance its step. For introductions, teach only step_title and supported_commands. When step_practiced is true, answer questions about the output and invite /next; do not assign a later exercise. If the learner explicitly asks ahead, discuss it as extra exploration, not a change to the saved course position.",
            "intent":intent,"learner_message":turn.text,"can_assess_current_practice":turn.can_assess,
            "actual_command_output":turn.output,"command_topic":turn.lab_topic,
            "verified_command_success":match turn.kind {TurnKind::Lab(passed)=>Some(passed),_=>None},
            "pending_review":turn.review_for,"due_learned_context":due,"post_learning_session":self.review_session,
            "review_policy":"Use this as context when it naturally fits; not as a separate quiz. Do not ask a review on the opening turn. No assessment on control messages, introductions, or mere assent."
        }).to_string())
    }

    fn receive(&mut self, result: Result<String>, turn: &Turn) -> Result<()> {
        let reply = result.and_then(|text| TeachingReply::parse(&text));
        match reply {
            Ok(reply) => self.apply_reply(reply, turn),
            Err(error) => {
                self.say("Connection to Mercury", &format!("I couldn't get the teacher's response: {error}. Your learning progress hasn't advanced. The explanation remains above; you can keep practicing commands or try your message again."));
                Ok(())
            }
        }
    }

    pub fn apply_reply(&mut self, reply: TeachingReply, turn: &Turn) -> Result<()> {
        if self.apply_scenario_reply(&reply, turn)? {
            return self.checkpoint();
        }
        self.apply_teaching_reply(reply, turn)
    }

    fn apply_teaching_reply(&mut self, reply: TeachingReply, turn: &Turn) -> Result<()> {
        let (mut next, learned) = self.assessed_progress(&reply, turn);
        let tx = self.store.db.unchecked_transaction()?;
        next.pending_review = self.review_context(&reply, turn)?;
        if learned {
            next = self
                .store
                .enroll_topic(&self.lesson().id, &next, &reply.evidence)?;
        }
        self.store.save_topic(&self.lesson().id, &next)?;
        let learner = (!matches!(turn.kind, TurnKind::Introduction)).then_some(turn.text.as_str());
        self.store
            .save_teaching_exchange(&self.lesson().id, learner, &reply.message)?;
        let mut messages = vec![format!("Mercury\n{}", reply.message)];
        if learned {
            messages.push("Coach\nYou've worked through this concept. We can keep exploring it, or /next introduces the next one. I'll revisit it in conversation later.".into());
        }
        let checkpoint = self.checkpoint_reply(&messages)?;
        tx.commit()?;
        self.progress = next;
        self.transcript.extend(messages);
        self.saved_messages = self.transcript.len();
        self.last_checkpoint = checkpoint;
        Ok(())
    }

    fn assessed_progress(
        &self,
        reply: &TeachingReply,
        turn: &Turn,
    ) -> (crate::teaching_store::TopicProgress, bool) {
        let mut next = self.progress.clone();
        next.ready |= self.demonstrated_topic(reply, turn);
        next.practice_offered |=
            !matches!(turn.kind, TurnKind::Introduction) && reply.practice_offered;
        let learned = next.ready
            && next.learned_at.is_none()
            && self.lesson().kind != "command"
            && self.progress.step + 1 == curriculum::step_count(self.lesson());
        (next, learned)
    }

    fn review_context(&self, reply: &TeachingReply, turn: &Turn) -> Result<Option<String>> {
        let mut pending = self.progress.pending_review.clone();
        if let Some(review) = &reply.review_result {
            if valid_review(review, turn, &self.store.teaching_due()?) {
                let score = if review.outcome == Outcome::Remembered {
                    4
                } else {
                    1
                };
                self.store.review(&review.topic_id, score, &turn.text)?;
                self.store.db.execute(
                    "UPDATE teaching_topics SET notes=?1 WHERE topic_id=?2",
                    rusqlite::params![review.evidence, review.topic_id],
                )?;
                pending = None;
            }
        }
        if matches!(turn.kind, TurnKind::Introduction) || pending.is_some() {
            return Ok(pending);
        }
        let due = self.store.teaching_due()?;
        Ok(reply
            .review_prompt_for
            .clone()
            .filter(|id| due.iter().any(|topic| &topic.topic_id == id)))
    }

    fn demonstrated_topic(&self, reply: &TeachingReply, turn: &Turn) -> bool {
        self.progress.learned_at.is_none()
            && (self.lesson().kind != "walkthrough"
                || self.lab_practiced.as_ref()
                    == Some(&(self.lesson().id.clone(), self.progress.step)))
            && self.lesson().kind != "command"
            && turn.can_assess
            && matches!(turn.kind, TurnKind::Conversation)
            && turn.review_for.is_none()
            && reply.assessment == Assessment::Understood
            && !reply.evidence.trim().is_empty()
    }
}

fn valid_review(
    review: &crate::teaching_protocol::ReviewResult,
    turn: &Turn,
    due: &[DueTopic],
) -> bool {
    if matches!(turn.kind, TurnKind::Introduction | TurnKind::Practice) {
        return false;
    }
    if !crate::teaching_protocol::learner_answer(&turn.text) || review.evidence.trim().is_empty() {
        return false;
    }
    if turn.review_for.as_deref() != Some(&review.topic_id)
        || !due.iter().any(|topic| topic.topic_id == review.topic_id)
    {
        return false;
    }
    if turn
        .lab_topic
        .as_ref()
        .is_some_and(|id| id != &review.topic_id)
    {
        return false;
    }
    if review.outcome == Outcome::Remembered && !crate::teaching_protocol::substantive(&turn.text) {
        return false;
    }
    !matches!(
        (&turn.kind, &review.outcome),
        (TurnKind::Lab(false), Outcome::Remembered)
    )
}

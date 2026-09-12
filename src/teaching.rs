use crate::{
    coach::Coach, teaching_curriculum as curriculum, teaching_store::TopicProgress, Lesson, Store,
};
use anyhow::{ensure, Result};
use std::collections::VecDeque;
use tempfile::TempDir;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum TurnKind {
    Introduction,
    Conversation,
    Practice,
    Lab(bool),
    Scenario {
        id: String,
        evaluate: bool,
        verified: bool,
    },
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Turn {
    pub text: String,
    pub kind: TurnKind,
    pub can_assess: bool,
    pub review_for: Option<String>,
    pub output: String,
    pub lab_topic: Option<String>,
}

pub struct Teacher {
    pub(super) lab: crate::teaching_scenarios::LabState,
    pub(super) lab_practiced: Option<(String, usize)>,
    pub(super) lab_worker: Option<crate::teaching_scenarios::Worker>,
    pub lessons: Vec<Lesson>,
    pub(super) catalog: Vec<Lesson>,
    pub index: usize,
    pub progress: TopicProgress,
    pub input: String,
    pub transcript: Vec<String>,
    pub output: String,
    pub view: crate::teaching_view::ReadingView,
    pub coach: Coach,
    pub quit: bool,
    pub online: bool,
    pub(super) store: Store,
    pub(super) scope: String,
    pub(super) generation: usize,
    pub(super) queue: VecDeque<Turn>,
    pub(super) in_flight: Option<Turn>,
    pub(super) fixture: TempDir,
    pub(super) saved_messages: usize,
    pub(super) last_checkpoint: String,
    pub(super) review_session: Option<crate::reinforcement_session::ReviewSession>,
}

impl Teacher {
    pub fn new(store: Store, lessons: Vec<Lesson>, deck: Option<&str>) -> Result<Self> {
        store.prepare_teaching()?;
        let scope = deck.unwrap_or("all").to_owned();
        let catalog = lessons.clone();
        let selected: Vec<_> = lessons
            .into_iter()
            .filter(|lesson| match deck {
                None | Some("all") => true,
                Some("du") => lesson.id == crate::du_course::LESSON_ID,
                Some(name) => lesson.deck == name,
            })
            .collect();
        ensure!(!selected.is_empty(), "No topics match this deck");
        let saved = store.teaching_position(&scope)?;
        let index = crate::teaching_levels::start(&store, &selected, saved.as_deref())?;
        let progress = store.topic_progress(&selected[index].id)?;
        ensure!(
            progress.step < curriculum::step_count(&selected[index]),
            "Unsupported saved teaching step"
        );
        let fixture = crate::teaching_lab::practice_fixture()?;
        let mut app = Self {
            lab: Default::default(),
            lab_worker: None,
            lab_practiced: None,
            lessons: selected,
            catalog,
            index,
            progress,
            input: String::new(),
            transcript: Vec::new(),
            output: String::new(),
            view: crate::teaching_view::ReadingView::default(),
            coach: Coach::default(),
            quit: false,
            online: true,
            store,
            scope,
            generation: 0,
            queue: VecDeque::new(),
            in_flight: None,
            fixture,
            saved_messages: 0,
            last_checkpoint: String::new(),
            review_session: None,
        };
        app.restore_or_start()?;
        Ok(app)
    }

    pub fn lesson(&self) -> &Lesson {
        &self.lessons[self.index]
    }

    pub fn say(&mut self, who: &str, text: &str) {
        self.transcript.push(format!("{who}\n{text}"));
    }

    pub fn submit(&mut self, text: &str) -> Result<()> {
        if !text.trim().is_empty() {
            self.touch_review_session(chrono::Utc::now())?;
        }
        let result = self.submit_inner(text);
        self.checkpoint()?;
        result
    }

    fn submit_inner(&mut self, text: &str) -> Result<()> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        if self.lab.scenario
            && matches!(
                text.split_whitespace().next(),
                Some("/next" | "/topic" | "/level")
            )
        {
            self.say("Incident practice", "Your normal lesson is saved. Use /lab stop to finish this lab before continuing teaching; /hint supports the current incident.");
            return Ok(());
        }
        if !text.starts_with('/') {
            return self.respond(text);
        }
        self.instruction(text)
    }

    fn instruction(&mut self, text: &str) -> Result<()> {
        match text {
            "/quit" => self.quit = true,
            "/next" => self.next()?,
            "/topics" => self.show_topics(),
            "/scenarios" => self.scenarios()?,
            "/lab" => self.lab_control("")?,
            "/levels" => self.say("Learning levels", &crate::teaching_levels::overview(&self.store, &self.lessons)?),
            "/help" => self.say("Coach", "Talk normally, or type a supported lab command. /practice asks for a small guided exercise; /hint asks for a simpler explanation. /next continues once we've practiced this concept. /topic ID chooses a topic; /topics lists them. /levels shows the learning ladder; /level N explores a level. Mouse wheel scrolls the pane under the pointer. Tab selects a pane for PageUp/PageDown or arrows; Home/End jumps to its start/latest. Esc quits. Reviews are woven into teaching after you've learned a topic. /scenarios shows unlocked real incidents. /lab starts real Kubernetes or Docker practice. /solve EXPLANATION submits incident evidence."),
            "/practice" | "/hint" if self.lab.scenario => self.scenario_turn(text, None, "")?,
            "/practice" | "/hint" => self.enqueue(text, TurnKind::Practice)?,
            _ => self.named_instruction(text)?,
        }
        Ok(())
    }

    fn named_instruction(&mut self, text: &str) -> Result<()> {
        let (command, argument) = text.split_once(' ').unwrap_or((text, ""));
        match command {
            "/scenario" => self.start_scenario(argument)?,
            "/solve" => self.solve_scenario(argument)?,
            "/lab" => self.lab_control(argument)?,
            "/ask" if self.lab.scenario => self.scenario_turn(argument, None, "")?,
            "/ask" => self.enqueue(argument, TurnKind::Conversation)?,
            "/topic" => self.select_topic(argument)?,
            "/level" => self.select_level(argument)?,
            _ => self.say("Coach", "Try /help for the available controls."),
        }
        Ok(())
    }

    fn respond(&mut self, text: &str) -> Result<()> {
        let executable = text.split_whitespace().next().unwrap_or("");
        if matches!(executable, "kubectl" | "docker") {
            return self.lab_job("command", text, "");
        }
        if self.lab.scenario {
            return self.scenario_turn(text, None, "");
        }
        if executable == "du"
            || self
                .lesson()
                .allowed_commands
                .iter()
                .any(|allowed| allowed == executable)
        {
            return self.command(text);
        }
        self.enqueue(text, TurnKind::Conversation)
    }

    pub fn enqueue(&mut self, text: &str, kind: TurnKind) -> Result<()> {
        ensure!(
            self.queue.len() < 8,
            "Mercury has several messages waiting. Let it catch up, then resend this message"
        );
        self.say("You", text);
        let lab_topic = if matches!(kind, TurnKind::Lab(_)) {
            Some(if text.split_whitespace().next() == Some("du") {
                crate::du_course::LESSON_ID.into()
            } else if text.split_whitespace().next() == Some("kubectl") {
                "k8s-kubectl-basics".into()
            } else if text.split_whitespace().next() == Some("docker") {
                "docker-basics".into()
            } else {
                self.lesson().id.clone()
            })
        } else {
            None
        };
        self.queue.push_back(Turn {
            text: text.into(),
            can_assess: self.progress.practice_offered
                && crate::teaching_protocol::substantive(text),
            review_for: self.progress.pending_review.clone(),
            output: if lab_topic.is_some() {
                self.output.clone()
            } else {
                String::new()
            },
            lab_topic,
            kind,
        });
        Ok(())
    }

    fn next(&mut self) -> Result<()> {
        if !self.progress.ready {
            self.say(
                "Coach",
                "Let's work through a small example together before adding the next concept.",
            );
            return self.enqueue("Please guide me through one small practice example of what you just taught. Explain it first if I seem unsure.", TurnKind::Practice);
        }
        if self.progress.step + 1 < curriculum::step_count(self.lesson()) {
            let mut next = self.progress.clone();
            next.step += 1;
            next.ready = false;
            next.practice_offered = false;
            self.store.save_topic(&self.lesson().id, &next)?;
            self.progress = next;
            self.begin_new_context();
            return self.open_topic();
        }
        if self.progress.learned_at.is_none() {
            self.progress = self.store.learn_topic(
                &self.lesson().id,
                &self.progress,
                "Completed the incremental practice steps",
            )?;
        }
        if let Some(index) = crate::teaching_levels::choose(&self.store, &self.lessons)? {
            return self.switch(index);
        }
        self.say("Coach", "You've completed this collection. Keep exploring here; learned topics remain in our shared reinforcement pool.");
        Ok(())
    }

    fn select_level(&mut self, text: &str) -> Result<()> {
        let level: u8 = text.trim().parse()?;
        let options: Vec<_> = self
            .lessons
            .iter()
            .filter(|lesson| curriculum::level(lesson) == level)
            .cloned()
            .collect();
        ensure!(
            !options.is_empty(),
            "No topics in that level in this collection. Use /levels"
        );
        let chosen = crate::teaching_levels::choose(&self.store, &options)?.unwrap_or(0);
        self.select_topic(&options[chosen].id)
    }

    fn select_topic(&mut self, selection: &str) -> Result<()> {
        let selected = curriculum::select(&self.lessons, selection).ok_or_else(|| {
            anyhow::anyhow!("Unknown topic in this collection. Use /topics for IDs")
        })?;
        let index = self
            .lessons
            .iter()
            .position(|lesson| lesson.id == selected.id)
            .unwrap_or(0);
        self.switch(index)
    }

    fn switch(&mut self, index: usize) -> Result<()> {
        let progress = self.store.topic_progress(&self.lessons[index].id)?;
        self.store
            .set_teaching_position(&self.scope, &self.lessons[index].id)?;
        self.index = index;
        self.progress = progress;
        self.begin_new_context();
        self.open_topic()
    }

    fn begin_new_context(&mut self) {
        self.view = crate::teaching_view::ReadingView::default();
        self.generation += 1;
        self.in_flight = None;
        self.queue.clear();
        self.output.clear();
    }

    fn show_topics(&mut self) {
        let topics = self
            .lessons
            .iter()
            .map(|lesson| {
                format!(
                    "{} — {} ({})",
                    lesson.id,
                    curriculum::title(lesson),
                    lesson.deck
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.say("Learning path — /topic ID to choose", &topics);
    }
}

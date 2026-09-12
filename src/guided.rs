use std::fs;

use anyhow::Result;
use chrono::{DateTime, Utc};
use tempfile::TempDir;

use crate::{
    coach::Coach,
    course_store::Progress,
    du_command::DuCommand,
    du_course::{self, LAST_STEP, LESSON_ID},
    Store,
};

#[derive(PartialEq)]
pub enum Mode {
    Learning,
    Review,
    Practice,
}

pub struct Guided {
    pub progress: Progress,
    pub mode: Mode,
    pub input: String,
    pub transcript: Vec<String>,
    pub scroll: u16,
    pub coach: Coach,
    pub quit: bool,
    pub coaching_enabled: bool,
    pub next_review: Option<String>,
    store: Store,
    fixture: TempDir,
    generation: usize,
    previous_command: String,
    last_output: String,
}

impl Guided {
    pub fn new(store: Store) -> Result<Self> {
        let progress = store.course_progress()?;
        let next_review = store.next_du_review()?;
        let due = next_review
            .as_ref()
            .and_then(|date| date.parse::<DateTime<Utc>>().ok())
            .is_some_and(|date| date <= Utc::now());
        let mode = match (&progress.completed_at, due) {
            (None, _) => Mode::Learning,
            (Some(_), true) => Mode::Review,
            _ => Mode::Practice,
        };
        let mut app = Self {
            progress,
            mode,
            input: String::new(),
            transcript: Vec::new(),
            scroll: 0,
            coach: Coach::default(),
            quit: false,
            coaching_enabled: true,
            next_review,
            store,
            fixture: practice_fixture()?,
            generation: 0,
            previous_command: String::new(),
            last_output: String::new(),
        };
        app.welcome();
        Ok(app)
    }

    fn welcome(&mut self) {
        self.say("Coach", "Welcome. We'll build SRE habits: inspect, form a hypothesis, run a small command, then read the evidence. Type the commands yourself; I'll check what they actually do. /ask talks to Mercury; /topics shows the SWE-to-SRE roadmap.");
        self.say("Lab", "Practice tree: logs/ (including logs/archive/), cache/, empty/. These files stay the same throughout this session. Nothing here is your node's real disk usage.");
        if self.mode == Mode::Practice {
            self.say("Coach", "You've completed the du introduction. Free practice is open; it doesn't change your review schedule. Return when your review is due, or use /topics and /ask to explore SRE concepts.");
        }
        if self.progress.ready && self.mode == Mode::Learning {
            self.say(
                "Coach",
                "Your last step is saved as complete. Experiment more, or type /next to continue.",
            );
        }
    }

    pub fn previous_command(&self) -> &str {
        &self.previous_command
    }

    pub fn last_output(&self) -> &str {
        &self.last_output
    }

    pub fn say(&mut self, who: &str, text: &str) {
        self.transcript.push(format!("{who}\n{text}"));
        if self.transcript.len() > 60 {
            self.transcript.remove(0);
        }
        self.scroll = 0;
    }

    pub fn submit(&mut self, text: &str) -> Result<()> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        self.say("You", text);
        match text {
            "/next" => self.advance()?,
            "/hint" => self.hint()?,
            "/help" => self.say("Coach", "Type a du command and press Enter. /next continues after a successful step. /hint explains the current task. /ask QUESTION asks Mercury, who can search DevDocs and SearXNG. /topics shows the roadmap. Up recalls your last command; PageUp/PageDown scroll; Esc quits. Progress saves as you go."),
            "/topics" => self.say("SWE → SRE", crate::roadmap::ROADMAP),
            "/quit" => self.quit = true,
            question if question.starts_with("/ask ") => self.ask_question(&question[5..])?,
            unknown if unknown.starts_with('/') => self.say("Coach", "Unknown instruction. Type /help for controls, or /ask followed by your question."),
            command => self.run_command(command)?,
        }
        Ok(())
    }

    fn ask_question(&mut self, question: &str) -> Result<()> {
        if self.progress.step == LAST_STEP && self.mode != Mode::Practice {
            self.mark_assisted()?;
        }
        self.ask(question);
        Ok(())
    }

    fn run_command(&mut self, input: &str) -> Result<()> {
        self.previous_command = input.to_owned();
        let evaluated = DuCommand::parse(input).and_then(|command| {
            let output = command.run(self.fixture.path())?;
            Ok((
                du_course::meets_objective(self.progress.step, &command, &output),
                output,
            ))
        });
        let (passed, output) = match evaluated {
            Ok(result) => result,
            Err(error) => {
                self.say("Try again", &error.to_string());
                if self.progress.step == LAST_STEP && self.mode != Mode::Practice {
                    self.mark_assisted()?;
                }
                return Ok(());
            }
        };
        self.last_output = format!("$ {input}\n{output}\n[exit 0]");
        self.say("Actual output", &self.last_output.clone());
        self.assess(passed, input)?;
        self.ask("Explain the latest observed output in relation to this step. Offer one small optional experiment. Do not move ahead to the next step.");
        Ok(())
    }

    fn assess(&mut self, passed: bool, input: &str) -> Result<()> {
        if self.mode == Mode::Practice {
            self.say("Coach", "Command ran. Compare it with your previous output, or use /ask to discuss what changed.");
            return Ok(());
        }
        if self.mode == Mode::Review {
            return self.review(passed, input);
        }
        if !passed {
            self.say("Coach", "That command ran, but it doesn't demonstrate this step yet. Compare it with the task above, try again, or type /hint.");
            if self.progress.step == LAST_STEP {
                self.mark_assisted()?;
            }
            return Ok(());
        }
        self.progress.ready = true;
        if self.progress.step == LAST_STEP {
            self.progress = self.store.graduate(&self.progress, input)?;
            self.complete("Challenge complete. du is now in spaced repetition. Unassisted completion returns in one day; a challenge with help or retries returns in ten minutes.")?;
            return Ok(());
        }
        self.store.save_course(&self.progress)?;
        let step = du_course::step(self.progress.step);
        self.say("What changed", step.observation);
        self.say("Try something", step.experiment);
        Ok(())
    }

    fn review(&mut self, passed: bool, input: &str) -> Result<()> {
        let score = match (passed, self.progress.assisted) {
            (false, _) => 1,
            (true, true) => 3,
            _ => 4,
        };
        self.store.review(LESSON_ID, score, input)?;
        if passed {
            return self.complete("Review passed from your command and output. Your next review has been scheduled automatically.");
        }
        self.complete("This attempt missed the objective. A review is scheduled in ten minutes. You can practice with /hint now; practice won't push that review away.")
    }

    fn complete(&mut self, message: &str) -> Result<()> {
        self.mode = Mode::Practice;
        self.next_review = self.store.next_du_review()?;
        self.progress.assisted = false;
        self.store.save_course(&self.progress)?;
        self.say("Coach", message);
        Ok(())
    }

    fn advance(&mut self) -> Result<()> {
        if self.mode != Mode::Learning {
            self.say("Coach", "The guided introduction is complete. Keep practicing, ask a question, or use /topics for what comes next.");
            return Ok(());
        }
        if !self.progress.ready {
            self.say(
                "Coach",
                "Run a command that demonstrates this step first. /hint will help.",
            );
            return Ok(());
        }
        self.progress.step = (self.progress.step + 1).min(LAST_STEP);
        self.progress.ready = false;
        self.generation += 1;
        self.store.save_course(&self.progress)?;
        self.last_output.clear();
        self.say("Coach", du_course::step(self.progress.step).introduction);
        Ok(())
    }

    fn mark_assisted(&mut self) -> Result<()> {
        self.progress.assisted = true;
        self.store.save_course(&self.progress)
    }

    fn hint(&mut self) -> Result<()> {
        if self.progress.step == LAST_STEP && self.mode != Mode::Practice {
            self.mark_assisted()?;
        }
        self.say("Hint", du_course::step(self.progress.step).hint);
        Ok(())
    }

    fn ask(&mut self, question: &str) {
        if !self.coaching_enabled {
            return;
        }
        let step = du_course::step(self.progress.step);
        let prompt = format!(
            "Current lesson: {}\nTask: {}\nSuggested experiment at this step: {}\nFor automatic feedback, keep experiments within the task and this suggestion; do not introduce later flags yet.\nLatest command output: {}\nLearner request: {}",
            step.title, step.introduction, step.experiment, self.last_output, question
        );
        if !self
            .coach
            .ask(self.generation, prompt, self.store.memories("guided-du"))
        {
            self.say("Coach", "Mercury is still answering. You can keep running commands; repeat /ask when it finishes.");
        }
    }

    pub fn poll_coach(&mut self) {
        let Some(reply) = self.coach.poll() else {
            return;
        };
        if reply.generation != self.generation && reply.generation != usize::MAX {
            return;
        }
        match reply.text {
            Ok(text) => {
                self.store.remember("guided-du", "user", &reply.prompt);
                self.store.remember("guided-du", "assistant", &text);
                self.say("Mercury", &text);
            },
            Err(error) => self.say("Mercury", &format!("Couldn't get coaching: {error}. You can continue with the built-in lesson and /hint.")),
        }
    }
}

fn practice_fixture() -> Result<TempDir> {
    let fixture = TempDir::new()?;
    crate::make_fixture("disk_usage", fixture.path())?;
    fs::create_dir(fixture.path().join("logs/archive"))?;
    fs::write(
        fixture.path().join("logs/archive/old.log"),
        vec![b'x'; 32768],
    )?;
    Ok(fixture)
}

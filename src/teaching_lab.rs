use crate::teaching::{Teacher, TurnKind};
use anyhow::Result;
use tempfile::TempDir;

impl Teacher {
    pub(super) fn command(&mut self, text: &str) -> Result<()> {
        let du = text.split_whitespace().next() == Some("du");
        let (passed, output) = if du {
            self.run_du(text)
        } else if crate::teaching_curriculum::step(self.lesson(), self.progress.step).is_some() {
            crate::teaching_courses::run(self.lesson(), self.progress.step, text)
        } else {
            crate::run_lab(self.lesson(), text)
        };
        self.output = output;
        self.view.output.home();
        self.enqueue(text, TurnKind::Lab(passed))?;
        if !self.can_learn_from_command(du, passed) {
            return Ok(());
        }
        let mut next = self.progress.clone();
        next.ready = true;
        if next.step + 1 < crate::teaching_curriculum::step_count(self.lesson()) {
            self.store.save_topic(&self.lesson().id, &next)?;
            self.progress = next;
            let suggestion = if du {
                crate::du_course::step(self.progress.step).experiment
            } else {
                "You've tried this addition. Ask questions or use /next to build on it."
            };
            self.say("Coach", suggestion);
            return Ok(());
        }
        self.progress = self.store.learn_topic(&self.lesson().id, &next, text)?;
        self.say("Coach", "You've practiced this concept successfully. Ask more questions or use /next to build on it. I'll bring it back naturally in a later conversation.");
        Ok(())
    }

    fn can_learn_from_command(&self, du: bool, passed: bool) -> bool {
        passed
            && self.progress.pending_review.is_none()
            && self.progress.learned_at.is_none()
            && (!du || self.lesson().id == crate::du_course::LESSON_ID)
    }

    fn run_du(&self, text: &str) -> (bool, String) {
        let result = crate::du_command::DuCommand::parse(text).and_then(|command| {
            let output = command.run(self.fixture.path())?;
            let step =
                if self.progress.pending_review.as_deref() == Some(crate::du_course::LESSON_ID) {
                    4
                } else {
                    self.progress.step
                };
            Ok((
                crate::du_course::meets_objective(step, &command, &output),
                format!("$ {text}\n{output}\n[exit 0]"),
            ))
        });
        result.unwrap_or_else(|error| (false, format!("Command not completed: {error}")))
    }
}

pub(super) fn practice_fixture() -> Result<TempDir> {
    let fixture = TempDir::new()?;
    crate::make_fixture("disk_usage", fixture.path())?;
    std::fs::create_dir(fixture.path().join("logs/archive"))?;
    std::fs::write(
        fixture.path().join("logs/archive/old.log"),
        vec![b'x'; 32768],
    )?;
    Ok(fixture)
}

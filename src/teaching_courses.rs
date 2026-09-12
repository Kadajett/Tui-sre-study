use crate::{
    teaching::{Teacher, TurnKind},
    teaching_curriculum as curriculum, Lesson,
};
use anyhow::{ensure, Result};

pub fn run(lesson: &Lesson, step: usize, text: &str) -> (bool, String) {
    match allowed(lesson, text) {
        Ok(parts) => {
            let mut lab = lesson.clone();
            lab.checks = None;
            let (executed, output) = crate::run_lab(&lab, text);
            let matches_step = curriculum::step(lesson, step).is_some_and(|part| {
                part.commands
                    .iter()
                    .any(|command| shell_words::split(command).ok().as_ref() == Some(&parts))
            });
            (executed && matches_step, output)
        }
        Err(error) => (false, error.to_string()),
    }
}

fn allowed(lesson: &Lesson, text: &str) -> Result<Vec<String>> {
    let parts = shell_words::split(text)?;
    let note = lesson
        .teaching
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No guided command course"))?;
    ensure!(note.steps.iter().flat_map(|step| &step.commands).any(|command| shell_words::split(command).ok().as_ref() == Some(&parts)), "This practice course supports the commands shown in its steps. Ask Mercury to explain an alternative before trying it.");
    Ok(parts)
}

impl Teacher {
    pub(super) fn walkthrough(&mut self, text: &str) -> Result<()> {
        let part = curriculum::step(self.lesson(), self.progress.step)
            .ok_or_else(|| anyhow::anyhow!("No current walkthrough step"))?;
        ensure!(
            part.commands
                .iter()
                .any(|command| shell_words::split(command).ok() == shell_words::split(text).ok()),
            "Try the command in this step first; this is a guided Kubernetes example."
        );
        self.output = format!(
            "KUBERNETES PRACTICE EXAMPLE\nSample output — no cluster connection\n\n$ {text}\n{}",
            part.example
                .lines()
                .filter(|line| !line.starts_with("```"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        self.view.output.home();
        self.progress.practice_offered = true;
        self.store.save_topic(&self.lesson().id, &self.progress)?;
        self.enqueue(&format!("I opened the sample for `{text}`. Walk me through its output, then invite a small explanation in my own words."), TurnKind::Practice)
    }
}

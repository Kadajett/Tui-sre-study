use crate::{teaching_curriculum as curriculum, Lesson};
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

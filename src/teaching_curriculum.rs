use crate::Lesson;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct TeachingNote {
    pub title: String,
    #[serde(default = "default_level")]
    pub level: u8,
    #[serde(default)]
    pub steps: Vec<TeachingStep>,
    pub why: String,
    pub example: String,
}

#[derive(Clone, Deserialize)]
pub struct TeachingStep {
    pub title: String,
    pub explanation: String,
    pub example: String,
    pub goal: String,
    #[serde(default)]
    pub commands: Vec<String>,
}

fn default_level() -> u8 {
    3
}

pub fn level(lesson: &Lesson) -> u8 {
    lesson.teaching.as_ref().map(|note| note.level).unwrap_or(3)
}

pub fn step(lesson: &Lesson, index: usize) -> Option<&TeachingStep> {
    lesson.teaching.as_ref()?.steps.get(index)
}

pub fn step_count(lesson: &Lesson) -> usize {
    if lesson.id == crate::du_course::LESSON_ID {
        return 5;
    }
    lesson
        .teaching
        .as_ref()
        .map(|note| note.steps.len().max(1))
        .unwrap_or(1)
}

pub fn practice_goal(lesson: &Lesson, index: usize) -> &str {
    step(lesson, index)
        .map(|step| step.goal.as_str())
        .unwrap_or(&lesson.prompt)
}

pub fn title(lesson: &Lesson) -> &str {
    lesson
        .teaching
        .as_ref()
        .map(|note| note.title.as_str())
        .unwrap_or(&lesson.id)
}

pub fn introduction(lesson: &Lesson, step: usize) -> String {
    if lesson.id == crate::du_course::LESSON_ID {
        let lesson = crate::du_course::step(step.min(4));
        return format!(
            "{}\n\n{}\n\n{}",
            lesson.title, lesson.introduction, lesson.observation
        );
    }
    if let Some(part) = self::step(lesson, step) {
        let command = part
            .commands
            .first()
            .map(|command| format!("Type: `{command}`"))
            .unwrap_or_default();
        let mode = if lesson.kind == "walkthrough" {
            if lesson.deck == "kubernetes" {
                "Start `/lab kubernetes` once, then type the command below. It runs against real pods in the dedicated sre-practice namespace."
            } else {
                "Start `/lab docker` once, then type the command below. It runs against real practice containers."
            }
        } else if part.commands.is_empty() {
            "This is a worked example to discuss. Ask a question or use /practice for a guided application."
        } else {
            "Run this in the input below. It executes inside the practice container."
        };
        return format!(
            "### {}\n\n{}\n\nWorked example:\n{}\n\n{}\n\n{}",
            part.title, part.explanation, part.example, command, mode
        );
    }
    let Some(note) = &lesson.teaching else {
        return format!("Let's build this concept together.\n\n{}\n\nAsk anything that is unclear. When you're ready, type /practice and we'll work through a small example.", lesson.answer);
    };
    format!("{}\n\nWhy this matters: {}\n\n{}\n\nWorked example: {}\n\nTake your time. Ask questions in ordinary language. /practice starts a small exercise together.", note.title, note.why, lesson.answer, note.example)
}

pub fn select<'a>(lessons: &'a [Lesson], selection: &str) -> Option<&'a Lesson> {
    lessons
        .iter()
        .find(|lesson| lesson.id == selection)
        .or_else(|| lessons.iter().find(|lesson| lesson.deck == selection))
}

use crate::{teaching_curriculum as curriculum, Lesson, Store};
use anyhow::Result;

pub fn name(level: u8) -> &'static str {
    match level {
        1 => "Terminal foundations",
        2 => "Networks & Kubernetes basics",
        3 => "Operating a service",
        _ => "Reliability & system design",
    }
}

pub fn candidates(store: &Store, lessons: &[Lesson]) -> Result<Vec<usize>> {
    let mut unlearned = Vec::new();
    for (index, lesson) in lessons.iter().enumerate() {
        if store.topic_progress(&lesson.id)?.learned_at.is_none() {
            unlearned.push(index);
        }
    }
    let easiest = unlearned
        .iter()
        .map(|index| curriculum::level(&lessons[*index]))
        .min();
    Ok(unlearned
        .into_iter()
        .filter(|index| Some(curriculum::level(&lessons[*index])) == easiest)
        .collect())
}

pub fn choose(store: &Store, lessons: &[Lesson]) -> Result<Option<usize>> {
    let options = candidates(store, lessons)?;
    if options.is_empty() {
        return Ok(None);
    }
    let choice: usize =
        store
            .db
            .query_row("SELECT abs(random() % ?1)", [options.len()], |row| {
                row.get(0)
            })?;
    Ok(options.get(choice).copied())
}

pub fn start(store: &Store, lessons: &[Lesson], saved: Option<&str>) -> Result<usize> {
    if let Some(index) = lessons
        .iter()
        .position(|lesson| Some(lesson.id.as_str()) == saved)
    {
        return Ok(index);
    }
    Ok(unfinished(store, lessons)?
        .or(choose(store, lessons)?)
        .unwrap_or(0))
}

fn unfinished(store: &Store, lessons: &[Lesson]) -> Result<Option<usize>> {
    for (index, lesson) in lessons.iter().enumerate() {
        let progress = store.topic_progress(&lesson.id)?;
        if progress.learned_at.is_none()
            && (progress.step > 0 || progress.ready || progress.practice_offered)
        {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

pub fn overview(store: &Store, lessons: &[Lesson]) -> Result<String> {
    let mut rows = Vec::new();
    for level in 1..=4 {
        let group: Vec<_> = lessons
            .iter()
            .filter(|lesson| curriculum::level(lesson) == level)
            .collect();
        let mut learned = 0;
        for lesson in &group {
            learned += usize::from(store.topic_progress(&lesson.id)?.learned_at.is_some());
        }
        rows.push(format!(
            "**Level {level} · {}** — {learned}/{} learned\n{}",
            name(level),
            group.len(),
            group
                .iter()
                .map(|lesson| format!("`{}`", lesson.id))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(format!("{}\n\nFresh picks come from the easiest unfinished level. /level N explores a level; /topic ID chooses a command. Every learned topic joins the same reinforcement pool.", rows.join("\n\n")))
}

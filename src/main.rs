mod chat_store;
mod coach;
mod course_store;
mod diagnostics;
mod du_command;
mod du_course;
mod guided;
mod guided_terminal;
#[cfg(test)]
mod guided_tests;
mod guided_ui;
mod lab_checks;
mod lab_commands;
mod lab_docker;
mod lab_kubernetes;
mod lab_runtime;
mod lab_server;
mod references;
mod reinforcement_session;
mod roadmap;
mod scenario_catalog;
#[cfg(test)]
mod scenario_tests;
mod teaching;
mod teaching_avatar;
#[cfg(test)]
mod teaching_course_tests;
mod teaching_courses;
mod teaching_curriculum;
mod teaching_dialogue;
mod teaching_lab;
mod teaching_levels;
mod teaching_markdown;
mod teaching_messages;
mod teaching_palette;
mod teaching_persistence;
#[cfg(test)]
mod teaching_persistence_tests;
mod teaching_protocol;
mod teaching_scenario_history;
mod teaching_scenarios;
mod teaching_store;
mod teaching_syntax;
mod teaching_terminal;
#[cfg(test)]
mod teaching_tests;
mod teaching_ui;
mod teaching_view;
#[cfg(test)]
mod teaching_view_tests;
mod teaching_wrap;

use std::{
    env, fs,
    io::{self, Stdout},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Terminal,
};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::json;
use tempfile::TempDir;

const ACCENT: Color = teaching_palette::VIOLET;
const CYAN: Color = teaching_palette::CYAN;
const MUTED: Color = teaching_palette::MUTED;

#[derive(Clone, Deserialize)]
struct Lesson {
    id: String,
    deck: String,
    kind: String,
    prompt: String,
    answer: String,
    fixture: Option<String>,
    #[serde(default)]
    allowed_commands: Vec<String>,
    checks: Option<Checks>,
    #[serde(default)]
    keyword_groups: Vec<Vec<String>>,
    minimum_groups: Option<usize>,
    #[serde(default)]
    teaching: Option<teaching_curriculum::TeachingNote>,
}

#[derive(Clone, Deserialize)]
struct Checks {
    #[serde(default)]
    exit: i32,
    #[serde(default)]
    contains: Vec<String>,
}

struct Store {
    db: Connection,
}

impl Store {
    fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let db = Connection::open(path)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS reviews(
            lesson_id TEXT PRIMARY KEY, repetitions INTEGER NOT NULL DEFAULT 0,
            interval_days INTEGER NOT NULL DEFAULT 0, ease REAL NOT NULL DEFAULT 2.5,
            due_at TEXT NOT NULL, last_score INTEGER, attempts INTEGER NOT NULL DEFAULT 0,
            correct INTEGER NOT NULL DEFAULT 0, last_answer TEXT);
            CREATE TABLE IF NOT EXISTS tutor_memory(
            id INTEGER PRIMARY KEY, lesson_id TEXT NOT NULL, role TEXT NOT NULL,
            content TEXT NOT NULL, created_at TEXT NOT NULL);",
        )?;
        Ok(Self { db })
    }

    fn due_key(&self, id: &str) -> (u8, String) {
        self.db
            .query_row("SELECT due_at FROM reviews WHERE lesson_id=?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .map(|due| {
                (
                    if due
                        .parse::<DateTime<Utc>>()
                        .map(|d| d <= Utc::now())
                        .unwrap_or(true)
                    {
                        0
                    } else {
                        2
                    },
                    due,
                )
            })
            .unwrap_or((1, String::new()))
    }

    fn review(&self, id: &str, score: i32, answer: &str) -> Result<()> {
        let old = self
            .db
            .query_row(
                "SELECT repetitions,interval_days,ease FROM reviews WHERE lesson_id=?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, i32>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, f64>(2)?,
                    ))
                },
            )
            .unwrap_or((0, 0, 2.5));
        let (mut repetitions, mut interval, mut ease) = old;
        let due = if score < 3 {
            repetitions = 0;
            interval = 0;
            Utc::now() + chrono::Duration::minutes(10)
        } else {
            repetitions += 1;
            interval = if repetitions == 1 {
                1
            } else if repetitions == 2 {
                6
            } else {
                ((interval as f64 * ease).round() as i64).max(1)
            };
            Utc::now() + chrono::Duration::days(interval)
        };
        let miss = (5 - score) as f64;
        ease = (ease + 0.1 - miss * (0.08 + miss * 0.02)).max(1.3);
        self.db.execute(
            "INSERT INTO reviews VALUES(?1,?2,?3,?4,?5,?6,1,?7,?8)
            ON CONFLICT(lesson_id) DO UPDATE SET repetitions=excluded.repetitions,
            interval_days=excluded.interval_days,ease=excluded.ease,due_at=excluded.due_at,
            last_score=excluded.last_score,attempts=reviews.attempts+1,
            correct=reviews.correct+excluded.correct,last_answer=excluded.last_answer",
            params![
                id,
                repetitions,
                interval,
                ease,
                due.to_rfc3339(),
                score,
                i32::from(score >= 3),
                answer
            ],
        )?;
        Ok(())
    }

    fn stats(&self) -> (i64, i64, i64) {
        self.db
            .query_row(
                "SELECT COUNT(*),COALESCE(SUM(attempts),0),COALESCE(SUM(correct),0) FROM reviews",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap_or_default()
    }

    fn memories(&self, id: &str) -> Vec<serde_json::Value> {
        let mut stmt = match self.db.prepare(
            "SELECT role,content FROM tutor_memory WHERE lesson_id=?1 ORDER BY id DESC LIMIT 6",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let mut rows: Vec<_> = stmt
            .query_map([id], |r| {
                Ok(json!({"role":r.get::<_,String>(0)?,"content":r.get::<_,String>(1)?}))
            })
            .ok()
            .map(|x| x.filter_map(|item| item.ok()).collect())
            .unwrap_or_default();
        rows.reverse();
        rows
    }

    fn remember(&self, id: &str, role: &str, content: &str) {
        let _ = self.db.execute(
            "INSERT INTO tutor_memory(lesson_id,role,content,created_at) VALUES(?1,?2,?3,?4)",
            params![id, role, content, Utc::now().to_rfc3339()],
        );
    }
}

#[derive(PartialEq)]
enum Phase {
    Answering,
    Reviewing,
}

struct App {
    lessons: Vec<Lesson>,
    index: usize,
    input: String,
    phase: Phase,
    result: String,
    passed: bool,
    tutor_text: String,
    store: Store,
    quit: bool,
}

impl App {
    fn lesson(&self) -> &Lesson {
        &self.lessons[self.index]
    }
    fn submit(&mut self) {
        let lesson = self.lesson().clone();
        let (passed, result) = if lesson.kind == "command" {
            run_lab(&lesson, &self.input)
        } else {
            check_recall(&lesson, &self.input)
        };
        self.passed = passed;
        self.result = result;
        self.phase = Phase::Reviewing;
    }
    fn grade(&mut self, score: i32) {
        let id = self.lesson().id.clone();
        if let Err(error) = self.store.review(&id, score, &self.input) {
            self.result = format!("Could not save review: {error}. Retry your grade to save it.");
            self.tutor_text.clear();
            return;
        }
        if self.index + 1 >= self.lessons.len() {
            self.quit = true;
        } else {
            self.index += 1;
            self.input.clear();
            self.result.clear();
            self.tutor_text.clear();
            self.phase = Phase::Answering;
        }
    }
    fn skip(&mut self) {
        if self.index + 1 >= self.lessons.len() {
            self.quit = true;
            return;
        }
        self.index += 1;
        self.input.clear();
        self.result.clear();
        self.tutor_text.clear();
        self.phase = Phase::Answering;
    }
    fn ask_tutor(&mut self) {
        self.tutor_text = tutor(self.lesson(), &self.input, &self.result, &self.store)
            .unwrap_or_else(|e| format!("Tutor unavailable: {e}"));
    }
}

fn check_recall(lesson: &Lesson, answer: &str) -> (bool, String) {
    let lower = answer.to_lowercase();
    let matched = lesson
        .keyword_groups
        .iter()
        .filter(|group| {
            group
                .iter()
                .any(|word| lower.contains(&word.to_lowercase()))
        })
        .count();
    let needed = lesson
        .minimum_groups
        .unwrap_or(lesson.keyword_groups.len().max(1));
    (
        matched >= needed,
        format!(
            "Matched {matched}/{} key ideas (need {needed}).",
            lesson.keyword_groups.len()
        ),
    )
}

fn make_fixture(name: &str, root: &Path) -> Result<()> {
    match name {
        "disk_usage" => { for (dir,sizes) in [("logs",vec![8192,2048]),("cache",vec![16384]),("empty",vec![])] { let p=root.join(dir);fs::create_dir(&p)?;for (i,size) in sizes.into_iter().enumerate(){fs::write(p.join(format!("file{i}.bin")),vec![b'x';size])?;} } },
        "log_search" => fs::write(root.join("app.log"),"INFO boot\nERROR database timeout\nINFO retry\nERROR upstream 502\n")?,
        "json_logs" => fs::write(root.join("events.jsonl"),"{\"level\":\"info\",\"route\":\"/health\"}\n{\"level\":\"error\",\"route\":\"/checkout\"}\n{\"level\":\"info\",\"route\":\"/orders\"}\n")?,
        "permissions" => { use std::os::unix::fs::PermissionsExt; let p=root.join("deploy.sh");fs::write(&p,"#!/bin/sh\necho deployed\n")?;fs::set_permissions(p,fs::Permissions::from_mode(0o640))?; },
        "network" => {},
        "processes" => fs::write(root.join("README"),"Inspect the live process table.")?,
        other => anyhow::bail!("unknown fixture {other}"),
    }
    Ok(())
}

fn run_lab(lesson: &Lesson, answer: &str) -> (bool, String) {
    if lesson.fixture.as_deref() == Some("disk_usage") {
        return du_command::review(answer);
    }
    if [";", "&&", "||", "|", ">", "<", "`", "$(", "\n"]
        .iter()
        .any(|x| answer.contains(x))
    {
        return (
            false,
            "Rejected: shell operators are not allowed. Build one direct command.".into(),
        );
    }
    let parts = match shell_words::split(answer) {
        Ok(p) => p,
        Err(e) => return (false, format!("Could not parse command: {e}")),
    };
    if parts.is_empty() || !lesson.allowed_commands.contains(&parts[0]) {
        return (
            false,
            format!("Allowed command: {}", lesson.allowed_commands.join(", ")),
        );
    }
    if parts[1..].iter().any(|p| {
        p.starts_with('/')
            || Path::new(p)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
    }) {
        return (
            false,
            "Rejected: absolute paths and parent traversal are outside this lab.".into(),
        );
    }
    let temp = match TempDir::new() {
        Ok(t) => t,
        Err(e) => return (false, e.to_string()),
    };
    if let Err(e) = make_fixture(lesson.fixture.as_deref().unwrap_or(""), temp.path()) {
        return (false, e.to_string());
    }
    let output = match Command::new("timeout")
        .args(["--signal=KILL", "4"])
        .arg(&parts[0])
        .args(&parts[1..])
        .stdin(Stdio::null())
        .current_dir(temp.path())
        .env_clear()
        .env("PATH", env::var("PATH").unwrap_or_default())
        .env("LANG", "C.UTF-8")
        .output()
    {
        Ok(o) => o,
        Err(e) => return (false, format!("Command failed safely: {e}")),
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let checks = lesson.checks.as_ref();
    let mut passed = checks
        .map(|c| {
            output.status.code() == Some(c.exit)
                && c.contains
                    .iter()
                    .all(|s| text.to_lowercase().contains(&s.to_lowercase()))
        })
        .unwrap_or(output.status.success());
    if lesson.fixture.as_deref() == Some("permissions") {
        use std::os::unix::fs::PermissionsExt;
        passed &= fs::metadata(temp.path().join("deploy.sh"))
            .map(|meta| meta.permissions().mode() & 0o7777 == 0o740)
            .unwrap_or(false);
    }
    (
        passed,
        format!(
            "$ {answer}\n{}\n[exit {}]",
            text.trim_end(),
            output.status.code().unwrap_or(-1)
        ),
    )
}

fn tutor(lesson: &Lesson, answer: &str, result: &str, store: &Store) -> Result<String> {
    let key = env::var("OPENROUTER_API_KEY").context("set OPENROUTER_API_KEY to enable Mercury")?;
    anyhow::ensure!(
        !key.trim().is_empty(),
        "set OPENROUTER_API_KEY to enable Mercury"
    );
    let prompt = format!(
        "Question: {}\nLearner answer: {}\nObserved result: {}\nReference: {}",
        lesson.prompt, answer, result, lesson.answer
    );
    let mut messages = vec![
        json!({"role":"system","content":"You are a concise SRE coach. Teach from evidence, not certification trivia. Explain the learner's gap, give one mental model, then one tiny follow-up exercise. Never claim a command ran unless output proves it."}),
    ];
    messages.extend(store.memories(&lesson.id));
    messages.push(json!({"role":"user","content":prompt}));
    let response=reqwest::blocking::Client::builder().timeout(Duration::from_secs(30)).build()?.post("https://openrouter.ai/api/v1/chat/completions").bearer_auth(key)
        .json(&json!({"model":env::var("OPENROUTER_MODEL").unwrap_or_else(|_|"inception/mercury-2".into()),"messages":messages,"max_tokens":350})).send()?.error_for_status()?;
    let value: serde_json::Value = response.json()?;
    let text = value["choices"][0]["message"]["content"]
        .as_str()
        .context("missing tutor response")?
        .to_string();
    store.remember(&lesson.id, "user", &prompt);
    store.remember(&lesson.id, "assistant", &text);
    Ok(text)
}

fn ui(frame: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let (reviewed, attempts, correct) = app.store.stats();
    let ratio = (app.index + 1) as f64 / app.lessons.len().max(1) as f64;
    frame.render_widget(Gauge::default().block(Block::default().title(format!(" SRE TRAINER · {} · {}/{} · {reviewed} learned · {correct}/{attempts} successful ",app.lesson().deck,app.index+1,app.lessons.len())).borders(Borders::ALL)).gauge_style(Style::default().fg(ACCENT)).ratio(ratio),outer[0]);
    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if app.phase == Phase::Answering {
            [
                Constraint::Min(7),
                Constraint::Length(5),
                Constraint::Length(3),
            ]
        } else {
            [
                Constraint::Length(7),
                Constraint::Min(8),
                Constraint::Length(3),
            ]
        })
        .margin(1)
        .split(outer[1]);
    frame.render_widget(
        Paragraph::new(app.lesson().prompt.as_str())
            .wrap(Wrap { trim: false })
            .style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .title(format!(" {} · {} ", app.lesson().kind, app.lesson().id))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(CYAN)),
            ),
        main[0],
    );
    if app.phase == Phase::Answering {
        frame.render_widget(
            Paragraph::new(app.input.as_str()).block(
                Block::default()
                    .title(" Your answer ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT)),
            ),
            main[1],
        );
        frame.set_cursor_position((
            main[1].x + 1 + (app.input.chars().count() as u16).min(main[1].width.saturating_sub(3)),
            main[1].y + 1,
        ));
        frame.render_widget(
            Paragraph::new("Enter submit  •  Esc quit  •  Ctrl+N skip")
                .style(Style::default().fg(MUTED)),
            main[2],
        );
    } else {
        let feedback = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main[1]);
        let observed = if app.tutor_text.is_empty() {
            app.result.as_str()
        } else {
            app.tutor_text.as_str()
        };
        frame.render_widget(
            Paragraph::new(observed).wrap(Wrap { trim: false }).block(
                Block::default()
                    .title(if app.tutor_text.is_empty() {
                        " Observed output "
                    } else {
                        " Mercury coach "
                    })
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if app.passed {
                        ACCENT
                    } else {
                        Color::Red
                    })),
            ),
            feedback[0],
        );
        frame.render_widget(
            Paragraph::new(app.lesson().answer.as_str())
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(" Reference answer ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                ),
            feedback[1],
        );
        frame.render_widget(
            Paragraph::new(if app.passed {
                "Checks passed · Enter save & next · t ask tutor · Esc quit"
            } else {
                "Checks missed · Enter schedule retry & next · t ask tutor · Esc quit"
            })
            .style(Style::default().fg(MUTED)),
            main[2],
        );
    }
    frame.render_widget(
        Paragraph::new(
            "Real commands run in a disposable, unprivileged lab. Progress is saved in SQLite.",
        )
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP)),
        outer[2],
    );
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if diagnostics::run(&args)? {
        return Ok(());
    }
    let deck = args
        .windows(2)
        .find(|w| w[0] == "--deck")
        .map(|w| w[1].clone());
    let limit = args
        .windows(2)
        .find(|w| w[0] == "--new")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(12);
    let lessons_path = env::var("SRE_LESSONS").unwrap_or_else(|_| "data/lessons.json".into());
    let all: Vec<Lesson> = serde_json::from_str(&fs::read_to_string(lessons_path)?)?;
    let data = PathBuf::from(env::var("SRE_DATA_DIR").unwrap_or_else(|_| ".sre-trainer".into()));
    let store = Store::open(&data.join("progress.db"))?;
    if args.iter().any(|arg| arg == "--stats") {
        let (reviewed, attempts, correct) = store.stats();
        println!("Topics scheduled: {reviewed}\nAttempts: {attempts}\nSuccessful: {correct}");
        return Ok(());
    }
    if !args
        .iter()
        .any(|arg| arg == "--cards" || arg == "--du-drill")
    {
        return teaching_terminal::run(store, all, deck.as_deref());
    }
    if !args.iter().any(|arg| arg == "--cards")
        && deck
            .as_deref()
            .is_none_or(|name| name == "du" || name == "linux")
    {
        return guided_terminal::run(store);
    }
    let du_learned = store.course_progress()?.completed_at.is_some();
    let mut lessons: Vec<_> = all
        .into_iter()
        .filter(|l| deck.as_ref().map(|d| d == &l.deck).unwrap_or(true))
        .filter(|l| l.id != du_course::LESSON_ID || du_learned)
        .filter(|l| store.due_key(&l.id).0 < 2)
        .collect();
    lessons.sort_by_key(|l| store.due_key(&l.id));
    lessons.truncate(limit);
    if lessons.is_empty() {
        println!("No due or new lessons match this selection. Try another deck or return when reviews are due.");
        return Ok(());
    }
    let mut app = App {
        lessons,
        index: 0,
        input: String::new(),
        phase: Phase::Answering,
        result: String::new(),
        passed: false,
        tutor_text: String::new(),
        store,
        quit: false,
    };
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let run = (|| -> Result<()> {
        while !app.quit {
            terminal.draw(|f| ui(f, &app))?;
            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Esc => app.quit = true,
                        KeyCode::Char('n')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            app.skip()
                        }
                        KeyCode::Enter if app.phase == Phase::Answering => app.submit(),
                        KeyCode::Backspace if app.phase == Phase::Answering => {
                            app.input.pop();
                        }
                        KeyCode::Char(c) if app.phase == Phase::Answering => app.input.push(c),
                        KeyCode::Enter if app.phase == Phase::Reviewing => {
                            app.grade(if app.passed { 4 } else { 1 })
                        }
                        KeyCode::Char('t') if app.phase == Phase::Reviewing => app.ask_tutor(),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    })();
    restore_terminal(&mut terminal)?;
    run?;
    println!("Session saved.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lesson() -> Lesson {
        Lesson {
            id: "x".into(),
            deck: "linux".into(),
            kind: "command".into(),
            prompt: "".into(),
            answer: "".into(),
            fixture: Some("log_search".into()),
            allowed_commands: vec!["grep".into()],
            checks: Some(Checks {
                exit: 0,
                contains: vec!["upstream 502".into()],
            }),
            keyword_groups: vec![],
            minimum_groups: None,
            teaching: None,
        }
    }
    #[test]
    fn real_lab_runs() {
        let (p, o) = run_lab(&lesson(), "grep ERROR app.log");
        assert!(p);
        assert!(o.contains("upstream 502"));
    }
    #[test]
    fn shell_is_rejected() {
        assert!(!run_lab(&lesson(), "grep ERROR app.log | cat").0);
    }
    #[test]
    fn recall_checks_concepts() {
        let mut l = lesson();
        l.keyword_groups = vec![vec!["dns".into()], vec!["tcp".into()]];
        l.minimum_groups = Some(2);
        assert!(check_recall(&l, "DNS then TCP").0);
    }
}

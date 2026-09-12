use chrono::{DateTime, Utc};
use tempfile::TempDir;

use crate::{
    du_command::DuCommand,
    du_course,
    guided::{Guided, Mode},
    Store,
};

fn session(directory: &TempDir) -> Guided {
    let store = Store::open(&directory.path().join("progress.db")).unwrap();
    let mut app = Guided::new(store).unwrap();
    app.coaching_enabled = false;
    app
}

fn graduate(app: &mut Guided) {
    for command in [
        "du",
        "du -h",
        "du -h logs",
        "du -sh logs",
        "du -h --max-depth=1 .",
    ] {
        app.submit(command).unwrap();
        assert!(app.progress.ready, "Command should pass: {command}");
        app.submit("/next").unwrap();
    }
    assert_eq!(app.progress.step, du_course::LAST_STEP);
    assert!(app.progress.completed_at.is_none());
}

#[test]
fn learner_must_run_each_addition_before_moving_on_and_can_resume() {
    let dir = TempDir::new().unwrap();
    let mut app = session(&dir);
    app.submit("/next").unwrap();
    assert_eq!(app.progress.step, 0);
    app.submit("du -h").unwrap();
    assert!(
        !app.progress.ready,
        "Skipping straight to flags doesn't complete plain du"
    );
    app.submit("du").unwrap();
    assert!(app.progress.ready);
    drop(app);
    let mut resumed = session(&dir);
    assert!(resumed.progress.ready);
    resumed.submit("/next").unwrap();
    assert_eq!(resumed.progress.step, 1);
    assert!(!resumed.progress.ready);
    assert!(resumed.next_review.is_none());
}

#[test]
fn only_a_successful_final_challenge_enrolls_the_lesson() {
    let dir = TempDir::new().unwrap();
    let mut app = session(&dir);
    graduate(&mut app);
    let store = Store::open(&dir.path().join("progress.db")).unwrap();
    assert_eq!(store.stats(), (0, 0, 0));
    app.submit("du -hd1 .").unwrap();
    assert!(app.mode == Mode::Practice);
    assert_eq!(store.stats(), (1, 1, 1));
    let due: DateTime<Utc> = app.next_review.as_ref().unwrap().parse().unwrap();
    assert!((due - Utc::now()).num_hours() >= 23);
    app.submit("du -sh logs").unwrap();
    assert_eq!(
        store.stats(),
        (1, 1, 1),
        "Free practice must not reschedule or add reviews"
    );
    assert!(session(&dir).mode == Mode::Practice);
}

#[test]
fn challenge_hints_and_wrong_commands_require_earlier_reinforcement() {
    let dir = TempDir::new().unwrap();
    let mut app = session(&dir);
    graduate(&mut app);
    app.submit("du -h .").unwrap();
    assert!(app.progress.completed_at.is_none());
    assert!(app.next_review.is_none());
    app.submit("/hint").unwrap();
    app.submit("du --human-readable --max-depth 1 .").unwrap();
    let due: DateTime<Utc> = app.next_review.as_ref().unwrap().parse().unwrap();
    assert!((due - Utc::now()).num_minutes() <= 10);
    assert!((due - Utc::now()).num_minutes() >= 9);
}

#[test]
fn due_review_checks_the_command_and_schedules_misses_without_self_grading() {
    let dir = TempDir::new().unwrap();
    let mut app = session(&dir);
    graduate(&mut app);
    app.submit("du -hd1 .").unwrap();
    drop(app);
    let store = Store::open(&dir.path().join("progress.db")).unwrap();
    store
        .db
        .execute("UPDATE reviews SET due_at='2000-01-01T00:00:00Z'", [])
        .unwrap();
    let mut review = session(&dir);
    assert!(review.mode == Mode::Review);
    review.submit("du -sh logs").unwrap();
    assert!(review.mode == Mode::Practice);
    assert_eq!(store.stats(), (1, 2, 1));
    let due: DateTime<Utc> = review.next_review.as_ref().unwrap().parse().unwrap();
    assert!((due - Utc::now()).num_minutes() <= 10);
    review.submit("du -hd1 .").unwrap();
    assert_eq!(store.stats(), (1, 2, 1));
}

#[test]
fn different_flag_spellings_run_and_match_the_same_depth_objective() {
    let root = TempDir::new().unwrap();
    crate::make_fixture("disk_usage", root.path()).unwrap();
    std::fs::create_dir(root.path().join("logs/archive")).unwrap();
    for input in [
        "du -hd1 .",
        "du -h -d 1",
        "du --human-readable --max-depth=1 .",
    ] {
        let command = DuCommand::parse(input).unwrap();
        let output = command.run(root.path()).unwrap();
        assert!(du_course::meets_objective(5, &command, &output), "{input}");
    }
    let plain = DuCommand::parse("du -h .").unwrap();
    let output = plain.run(root.path()).unwrap();
    assert!(output.contains("logs/archive"));
    assert!(!du_course::meets_objective(5, &plain, &output));
}

#[test]
fn du_sandbox_rejects_shell_operators_host_paths_and_file_reading_options() {
    for input in [
        "du /",
        "du ../",
        "du --files0-from=/proc/self/environ",
        "du -h; id",
        "du -h | cat",
        "du $(id)",
        "sh -c du",
        "du -h /etc",
        "du -d99",
        "du -sh -d1 .",
        "du --exclude=logs .",
    ] {
        assert!(DuCommand::parse(input).is_err(), "Must reject {input}");
    }
    for input in [
        "du -ah logs",
        "du -sh logs cache empty",
        "du -hc .",
        "du -d 0 .",
    ] {
        assert!(DuCommand::parse(input).is_ok(), "Allow experiment {input}");
    }
}

#[test]
fn syntax_errors_leave_training_progress_and_reviews_untouched() {
    let dir = TempDir::new().unwrap();
    let mut app = session(&dir);
    app.submit("du --files0-from=/etc/passwd").unwrap();
    assert_eq!(app.progress.step, 0);
    assert!(!app.progress.ready);
    assert!(app.next_review.is_none());
    assert!(app.transcript.last().unwrap().contains("Try again"));
}

#[test]
fn screen_shows_the_current_command_then_the_next_addition() {
    let dir = TempDir::new().unwrap();
    let mut app = session(&dir);
    let backend = ratatui::backend::TestBackend::new(100, 32);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| crate::guided_ui::draw(frame, &app))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Meet du"));
    assert!(text.contains("Type: du"));
    app.submit("du").unwrap();
    terminal
        .draw(|frame| crate::guided_ui::draw(frame, &app))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Actual output"));
    assert!(text.contains("[exit 0]"));
    app.submit("/next").unwrap();
    terminal
        .draw(|frame| crate::guided_ui::draw(frame, &app))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Type: du -h"));
    assert!(text.contains("step 2/6"));
}

#[test]
fn existing_reviews_survive_course_upgrade_without_skipping_the_introduction() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("progress.db")).unwrap();
    store.review("linux-du-1", 4, "du -hd1 .").unwrap();
    store
        .review("k8s-first-loop", 4, "get describe logs")
        .unwrap();
    let mut app = session(&dir);
    assert!(app.mode == Mode::Learning);
    assert_eq!(app.progress.step, 0);
    graduate(&mut app);
    app.submit("du -hd1 .").unwrap();
    assert_eq!(store.stats(), (2, 3, 3));
    let due: DateTime<Utc> = app.next_review.as_ref().unwrap().parse().unwrap();
    assert_eq!((due - Utc::now()).num_hours(), 23);
}

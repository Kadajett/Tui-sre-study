use crate::{
    teaching::{Teacher, TurnKind},
    teaching_protocol::{Assessment, TeachingReply},
    Store,
};
use chrono::{Duration, TimeZone, Utc};
use tempfile::TempDir;

fn app(dir: &TempDir) -> Teacher {
    app_with_deck(dir, Some("du"))
}

fn app_with_deck(dir: &TempDir, deck: Option<&str>) -> Teacher {
    let lessons = serde_json::from_str(include_str!("../data/lessons.json")).unwrap();
    let mut app = Teacher::new(
        Store::open(&dir.path().join("progress.db")).unwrap(),
        lessons,
        deck,
    )
    .unwrap();
    app.online = false;
    app
}

fn settle(app: &mut Teacher) {
    app.queue.clear();
    app.in_flight = None;
    app.checkpoint().unwrap();
}

#[test]
fn halfway_through_du_restores_every_message_and_output_without_an_introduction() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    settle(&mut app);
    for command in ["du", "du -h"] {
        app.submit(command).unwrap();
        app.submit("/next").unwrap();
        settle(&mut app);
    }
    app.submit("du -h logs").unwrap();
    settle(&mut app);
    for i in 0..85 {
        app.say("Mercury", &format!("Saved explanation {i}"));
    }
    app.input = "What does the logs total include".into();
    app.view.conversation.top = 9;
    app.view.conversation.follow = false;
    app.checkpoint().unwrap();
    let transcript = app.transcript.clone();
    let output = app.output.clone();
    drop(app);
    let resumed = self::app(&dir);
    assert_eq!(resumed.progress.step, 2);
    assert!(resumed.progress.ready);
    assert_eq!(resumed.transcript, transcript);
    assert_eq!(resumed.output, output);
    assert_eq!(resumed.input, "What does the logs total include");
    assert_eq!(resumed.view.conversation.top, 9);
    assert!(!resumed.view.conversation.follow);
    assert!(resumed.queue.is_empty());
}

#[test]
fn submitted_questions_and_inflight_lab_evidence_resume_once_in_order() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    settle(&mut app);
    app.submit("du").unwrap();
    let lab = app.queue.pop_front().unwrap();
    app.in_flight = Some(lab);
    app.submit("Why is an empty directory zero here?").unwrap();
    app.checkpoint().unwrap();
    let transcript = app.transcript.clone();
    drop(app);
    let mut resumed = self::app(&dir);
    assert_eq!(resumed.queue.len(), 2);
    assert_eq!(resumed.transcript, transcript);
    let lab = resumed.queue.pop_front().unwrap();
    assert!(matches!(lab.kind, TurnKind::Lab(true)));
    assert!(lab.output.contains("[exit 0]"));
    resumed
        .apply_reply(
            TeachingReply {
                message: "The command measured allocated blocks.".into(),
                assessment: Assessment::Continue,
                evidence: String::new(),
                practice_offered: false,
                review_prompt_for: None,
                review_result: None,
            },
            &lab,
        )
        .unwrap();
    let transcript = resumed.transcript.clone();
    drop(resumed);
    let resumed = self::app(&dir);
    assert_eq!(resumed.transcript, transcript);
    assert_eq!(resumed.queue.len(), 1);
    assert_eq!(
        resumed.queue[0].text,
        "Why is an empty directory zero here?"
    );
}

#[test]
fn all_previously_saved_chat_is_imported_once_without_a_new_greeting() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("progress.db")).unwrap();
    store.prepare_teaching().unwrap();
    store.set_teaching_position("du", "linux-du-1").unwrap();
    for i in 0..30 {
        store
            .save_teaching_message("linux-du-1", "assistant", &format!("Old explanation {i}"))
            .unwrap();
    }
    let app = app(&dir);
    assert_eq!(app.transcript.len(), 30);
    assert!(app.queue.is_empty());
    drop(app);
    let resumed = self::app(&dir);
    assert_eq!(resumed.transcript.len(), 30);
    assert!(resumed.queue.is_empty());
}

#[test]
fn completed_topic_reopens_in_the_same_chat_instead_of_picking_a_new_lesson() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    settle(&mut app);
    app.progress = app
        .store
        .learn_topic("linux-du-1", &app.progress, "Completed practice")
        .unwrap();
    app.say("Mercury", "We can keep exploring your question.");
    app.checkpoint().unwrap();
    let transcript = app.transcript.clone();
    drop(app);
    let resumed = self::app(&dir);
    assert!(resumed.progress.learned_at.is_some());
    assert_eq!(resumed.transcript, transcript);
    assert!(resumed.queue.is_empty());
}

#[test]
fn reinforcement_sessions_roll_after_24_elapsed_hours_without_resetting_teaching() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    settle(&mut app);
    let start = Utc.with_ymd_and_hms(2026, 9, 12, 23, 30, 0).unwrap();
    app.touch_review_session(start).unwrap();
    assert!(app.review_session.is_none());
    app.progress = app
        .store
        .learn_topic("linux-du-1", &app.progress, "Completed practice")
        .unwrap();
    let transcript = app.transcript.clone();
    app.touch_review_session(start).unwrap();
    let first = app.review_session.as_ref().unwrap().id;
    app.touch_review_session(start + Duration::hours(23))
        .unwrap();
    assert_eq!(app.review_session.as_ref().unwrap().id, first);
    app.touch_review_session(start + Duration::hours(24))
        .unwrap();
    assert_ne!(app.review_session.as_ref().unwrap().id, first);
    assert_eq!(app.transcript, transcript);
    assert!(app.queue.is_empty());
}

#[test]
fn failed_chat_commit_rolls_back_assessment_and_keeps_the_question_recoverable() {
    let dir = TempDir::new().unwrap();
    let mut app = app_with_deck(&dir, None);
    app.submit("/topic sre-slos").unwrap();
    settle(&mut app);
    app.progress.practice_offered = true;
    app.submit(
        "An SLI measures successful requests; the SLO sets their target, leaving an error budget.",
    )
    .unwrap();
    let turn = app.queue.pop_front().unwrap();
    let transcript = app.transcript.clone();
    app.store.db.execute_batch("CREATE TRIGGER fail_chat BEFORE UPDATE ON teaching_resume BEGIN SELECT RAISE(ABORT,'simulated full disk'); END;").unwrap();
    let result = app.apply_reply(
        TeachingReply {
            message: "You explained the relationship.".into(),
            assessment: Assessment::Understood,
            evidence: "Connected measurement, objective, and budget".into(),
            practice_offered: false,
            review_prompt_for: None,
            review_result: None,
        },
        &turn,
    );
    assert!(result.is_err());
    assert_eq!(app.store.stats().0, 0);
    assert!(app
        .store
        .topic_progress("sre-slos")
        .unwrap()
        .learned_at
        .is_none());
    assert_eq!(app.transcript, transcript);
    app.store
        .db
        .execute_batch("DROP TRIGGER fail_chat")
        .unwrap();
    drop(app);
    let resumed = app_with_deck(&dir, None);
    assert_eq!(resumed.transcript, transcript);
    assert_eq!(resumed.queue.len(), 1);
    assert_eq!(resumed.queue[0].text, turn.text);
}

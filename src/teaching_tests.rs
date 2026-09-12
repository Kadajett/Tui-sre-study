use crate::{
    teaching::{Teacher, Turn},
    teaching_protocol::{Assessment, Outcome, ReviewResult, TeachingReply},
    Store,
};
use tempfile::TempDir;

fn app(dir: &TempDir, deck: Option<&str>) -> Teacher {
    let lessons: Vec<crate::Lesson> =
        serde_json::from_str(include_str!("../data/lessons.json")).unwrap();
    let store = Store::open(&dir.path().join("progress.db")).unwrap();
    store.prepare_teaching().unwrap();
    let scope = deck.unwrap_or("all");
    if store.teaching_position(scope).unwrap().is_none() {
        let first = lessons
            .iter()
            .find(|lesson| deck.is_none_or(|deck| lesson.deck == deck))
            .unwrap();
        store.set_teaching_position(scope, &first.id).unwrap();
    }
    let mut app = Teacher::new(store, lessons, deck).unwrap();
    app.online = false;
    app.queue.clear();
    app
}

fn reply(assessment: Assessment) -> TeachingReply {
    TeachingReply {
        message: "A reliability target connects a measurement to the experience users need.".into(),
        assessment,
        evidence:
            "The learner explained the measurement, target and error budget in their own words."
                .into(),
        practice_offered: false,
        review_prompt_for: None,
        review_result: None,
    }
}

fn answer(app: &mut Teacher, text: &str) -> Turn {
    app.submit(text).unwrap();
    app.queue.pop_back().unwrap()
}

fn learned_slo(app: &mut Teacher) {
    let turn = answer(app, "/practice");
    let mut prompt = reply(Assessment::Continue);
    prompt.practice_offered = true;
    app.apply_reply(prompt, &turn).unwrap();
    let turn = answer(app,"The SLI measures successful checkouts, the SLO is its target, and the remaining allowed failures are the error budget.");
    app.apply_reply(reply(Assessment::Understood), &turn)
        .unwrap();
}

#[test]
fn every_topic_has_an_introduction_and_example_before_practice() {
    let dir = TempDir::new().unwrap();
    let app = app(&dir, Some("sre"));
    assert!(app.transcript.join("\n").contains("Worked example:"));
    assert!(!app.progress.ready);
    assert!(app.progress.learned_at.is_none());
    assert!(app.store.teaching_due().unwrap().is_empty());
    assert!(app.lessons.iter().all(|lesson| lesson.teaching.is_some()));
}

#[test]
fn assent_next_and_unoffered_practice_never_enroll_topics() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir, Some("sre"));
    for text in ["okay", "/next", "What does SLI mean?"] {
        let turn = answer(&mut app, text);
        app.apply_reply(reply(Assessment::Understood), &turn)
            .unwrap();
        assert!(!app.progress.ready);
        assert_eq!(app.index, 0);
    }
    let turn = answer(&mut app, "/practice");
    let mut offered = reply(Assessment::Continue);
    offered.practice_offered = true;
    app.apply_reply(offered, &turn).unwrap();
    let turn = answer(&mut app, "got it");
    app.apply_reply(reply(Assessment::Understood), &turn)
        .unwrap();
    assert!(app.progress.learned_at.is_none());
}

#[test]
fn demonstrated_learning_enrolls_once_then_next_introduces_one_topic() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir, Some("sre"));
    learned_slo(&mut app);
    assert!(app.progress.learned_at.is_some());
    assert!(app.store.teaching_due().unwrap().is_empty());
    assert_eq!(app.store.stats(), (1, 0, 0));
    app.submit("/next").unwrap();
    assert_ne!(app.lesson().id, "sre-slos");
    let next_index = app.index;
    assert!(app.transcript.join("\n").contains("Worked example:"));
    assert!(app.progress.learned_at.is_none());
    drop(app);
    let resumed = self::app(&dir, Some("sre"));
    assert_eq!(resumed.index, next_index);
}

#[test]
fn unknown_legacy_quiz_answers_are_not_review_context() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("progress.db")).unwrap();
    store.review("sre-slos", 1, "I don't know").unwrap();
    store
        .db
        .execute("UPDATE reviews SET due_at='2000-01-01T00:00:00Z'", [])
        .unwrap();
    let app = app(&dir, Some("sre"));
    assert!(app.store.teaching_due().unwrap().is_empty());
    assert!(app.progress.learned_at.is_none());
}

#[test]
fn due_topics_feed_the_teacher_and_only_contextual_answers_reschedule_them() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir, Some("sre"));
    learned_slo(&mut app);
    app.store
        .db
        .execute("UPDATE reviews SET due_at='2000-01-01T00:00:00Z'", [])
        .unwrap();
    app.submit("/next").unwrap();
    let intro = app.queue.pop_front().unwrap();
    let context: serde_json::Value = serde_json::from_str(&app.context(&intro).unwrap()).unwrap();
    assert_eq!(
        context["due_learned_context"][0]["history"]["topic_id"],
        "sre-slos"
    );
    let mut opening = reply(Assessment::Continue);
    opening.review_prompt_for = Some("sre-slos".into());
    app.apply_reply(opening, &intro).unwrap();
    assert!(
        app.progress.pending_review.is_none(),
        "Opening must remain teaching"
    );
    let turn = answer(&mut app, "How do reliability objectives influence a page?");
    let mut invitation = reply(Assessment::Continue);
    invitation.review_prompt_for = Some("sre-slos".into());
    app.apply_reply(invitation, &turn).unwrap();
    assert_eq!(app.progress.pending_review.as_deref(), Some("sre-slos"));
    let turn=answer(&mut app,"The SLO sets our target, so an alert should tell us when user reliability threatens that target.");
    let mut assessment = reply(Assessment::Continue);
    assessment.review_result = Some(ReviewResult {
        topic_id: "sre-slos".into(),
        outcome: Outcome::Remembered,
        evidence: "Connected the learned SLO concept to alerting.".into(),
    });
    app.apply_reply(assessment, &turn).unwrap();
    assert!(app.store.teaching_due().unwrap().is_empty());
    assert!(
        app.progress.learned_at.is_none(),
        "Reviewing SLOs doesn't graduate the new paging topic"
    );
    assert_ne!(
        app.lesson().id,
        "sre-slos",
        "There is no separate review screen"
    );
    assert_eq!(app.store.stats(), (1, 1, 1));
}

#[test]
fn du_additions_still_execute_and_teach_before_enrollment() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir, None);
    for command in ["du", "du -h", "du -h logs", "du -sh logs"] {
        app.submit(command).unwrap();
        assert!(app.progress.ready);
        assert!(app.progress.learned_at.is_none());
        app.submit("/next").unwrap();
    }
    app.submit("du -hd1 .").unwrap();
    assert!(app.output.contains("[exit 0]"));
    assert!(app.progress.learned_at.is_some());
    app.submit("/next").unwrap();
    assert_ne!(app.lesson().id, "linux-du-1");
    assert_eq!(crate::teaching_curriculum::level(app.lesson()), 1);
    assert!(app.transcript.join("\n").contains("Worked example:"));
}

#[test]
fn malformed_teacher_metadata_cannot_mutate_learning() {
    assert!(TeachingReply::parse("not JSON").is_err());
    assert!(TeachingReply::parse(r#"{"message":"hello","assessment":"perfect","evidence":"","practice_offered":false,"review_prompt_for":null,"review_result":null}"#).is_err());
}

#[test]
fn asking_for_help_after_practice_is_not_mastery() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir, Some("sre"));
    app.progress.practice_offered = true;
    for text in [
        "Why is that an SLI?",
        "I don't know",
        "please explain that again",
    ] {
        let turn = answer(&mut app, text);
        app.apply_reply(reply(Assessment::Understood), &turn)
            .unwrap();
        assert!(app.progress.learned_at.is_none());
    }
}

#[test]
fn forgetting_a_learned_concept_schedules_support_without_graduating_the_new_topic() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir, Some("sre"));
    learned_slo(&mut app);
    app.store
        .db
        .execute("UPDATE reviews SET due_at='2000-01-01T00:00:00Z'", [])
        .unwrap();
    app.submit("/next").unwrap();
    app.progress.pending_review = Some("sre-slos".into());
    let turn = answer(&mut app, "I don't know");
    let mut response = reply(Assessment::Continue);
    response.review_result = Some(ReviewResult {
        topic_id: "sre-slos".into(),
        outcome: Outcome::NeedsHelp,
        evidence: "The learner needs the SLO concept explained again.".into(),
    });
    app.apply_reply(response.clone(), &turn).unwrap();
    assert!(app.progress.learned_at.is_none());
    assert!(app.progress.pending_review.is_none());
    assert_eq!(app.store.stats(), (1, 1, 0));
    let due: String = app
        .store
        .db
        .query_row(
            "SELECT due_at FROM reviews WHERE lesson_id='sre-slos'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let due = due.parse::<chrono::DateTime<chrono::Utc>>().unwrap();
    assert_eq!((due - chrono::Utc::now()).num_minutes(), 9);
    app.apply_reply(response, &turn).unwrap();
    assert_eq!(
        app.store.stats(),
        (1, 1, 0),
        "A repeated response must not count twice"
    );
}

#[test]
fn failed_conversation_save_rolls_back_mastery_and_schedule_together() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir, Some("sre"));
    app.progress.practice_offered = true;
    let turn=answer(&mut app,"The SLI measures successful checkouts, the SLO sets its target and the budget is its allowed failures.");
    app.store.db.execute_batch("CREATE TRIGGER reject_teaching_memory BEFORE INSERT ON tutor_memory BEGIN SELECT RAISE(ABORT,'simulated write failure'); END;").unwrap();
    assert!(app
        .apply_reply(reply(Assessment::Understood), &turn)
        .is_err());
    assert!(app.progress.learned_at.is_none());
    assert!(!app.progress.ready);
    assert!(app
        .store
        .topic_progress("sre-slos")
        .unwrap()
        .learned_at
        .is_none());
    assert_eq!(app.store.stats(), (0, 0, 0));
}

#[test]
fn teacher_screen_opens_with_explanation_instead_of_a_quiz() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir, Some("sre"));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(110, 36)).unwrap();
    terminal
        .draw(|frame| crate::teaching_ui::draw(frame, &mut app))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Worked example:"));
    assert!(text.contains("Talk to Mercury"));
    assert!(!text.contains("Your answer"));
    assert!(!text.contains("DU REVIEW"));
}

#[test]
fn an_unfinished_du_walkthrough_resumes_at_its_saved_addition() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("progress.db")).unwrap();
    store.course_progress().unwrap();
    store
        .save_course(&crate::course_store::Progress {
            step: 2,
            ready: true,
            assisted: false,
            completed_at: None,
        })
        .unwrap();
    let mut app = app(&dir, None);
    assert_eq!(app.progress.step, 2);
    assert!(app.progress.ready);
    assert!(app.progress.learned_at.is_none());
    app.submit("/next").unwrap();
    assert_eq!(app.progress.step, 3);
    assert!(!app.progress.ready);
}

#[test]
fn introductions_use_prepared_material_and_conversation_can_retrieve_references() {
    assert!(!crate::coach::allow_reference_tools(
        r#"{"intent":"explain_first_no_questions"}"#,
        true
    )
    .unwrap());
    assert!(
        crate::coach::allow_reference_tools(r#"{"intent":"respond_to_learner"}"#, true).unwrap()
    );
    assert!(crate::coach::allow_reference_tools("legacy coach question", false).unwrap());
    assert!(crate::coach::allow_reference_tools("invalid context", true).is_err());
}

use crate::{
    teaching::Teacher,
    teaching_curriculum as curriculum, teaching_levels,
    teaching_protocol::{Assessment, TeachingReply},
    Lesson, Store,
};
use tempfile::TempDir;

fn catalog() -> Vec<Lesson> {
    serde_json::from_str(include_str!("../data/lessons.json")).unwrap()
}

fn app(dir: &TempDir) -> Teacher {
    let mut app = Teacher::new(
        Store::open(&dir.path().join("progress.db")).unwrap(),
        catalog(),
        None,
    )
    .unwrap();
    app.online = false;
    app.queue.clear();
    app
}

#[test]
fn fresh_choices_are_random_beginner_commands_and_advanced_levels_wait() {
    let dir = TempDir::new().unwrap();
    let app = app(&dir);
    let choices = teaching_levels::candidates(&app.store, &app.lessons).unwrap();
    assert_eq!(choices.len(), 6);
    assert!(choices
        .iter()
        .all(|index| curriculum::level(&app.lessons[*index]) == 1
            && app.lessons[*index].kind == "command"));
    let mut selected = std::collections::HashSet::new();
    for _ in 0..24 {
        let index = teaching_levels::choose(&app.store, &app.lessons)
            .unwrap()
            .unwrap();
        assert!(choices.contains(&index));
        selected.insert(index);
    }
    assert!(selected.len() > 1);
    for index in choices {
        app.store
            .learn_topic(
                &app.lessons[index].id,
                &Default::default(),
                "Test completion",
            )
            .unwrap();
    }
    let next = teaching_levels::choose(&app.store, &app.lessons)
        .unwrap()
        .unwrap();
    assert_eq!(curriculum::level(&app.lessons[next]), 2);
}

#[test]
fn each_command_course_requires_every_addition_before_joining_the_pool() {
    for id in [
        "linux-grep-1",
        "linux-jq-1",
        "linux-chmod-1",
        "linux-ps-1",
        "network-ip-addr",
    ] {
        let dir = TempDir::new().unwrap();
        let mut app = app(&dir);
        app.submit(&format!("/topic {id}")).unwrap();
        let steps = app.lesson().teaching.as_ref().unwrap().steps.clone();
        app.submit(&steps.last().unwrap().commands[0]).unwrap();
        assert!(
            !app.progress.ready,
            "An advanced command must not skip the start of {id}"
        );
        app.queue.clear();
        for (index, step) in steps.iter().enumerate() {
            app.submit(&step.commands[0]).unwrap();
            assert!(app.progress.ready, "{id} step {index}: {}", app.output);
            let last = index + 1 == steps.len();
            assert_eq!(app.progress.learned_at.is_some(), last, "{id} step {index}");
            if !last {
                assert_eq!(app.store.stats().0, 0);
                app.submit("/next").unwrap();
                assert_eq!(app.progress.step, index + 1);
            }
            app.queue.clear();
        }
        assert_eq!(app.store.stats(), (1, 0, 0));
    }
}

#[test]
fn networking_course_cannot_modify_addresses_or_inspect_host_files() {
    let lessons = catalog();
    let ip = lessons
        .iter()
        .find(|lesson| lesson.id == "network-ip-addr")
        .unwrap();
    for command in [
        "ip addr flush dev eth0",
        "ip link set eth0 down",
        "ip -batch /etc/passwd",
        "ip addr; echo wrong",
    ] {
        let (passed, output) = crate::teaching_courses::run(ip, 0, command);
        assert!(!passed);
        assert!(output.contains("supports the commands"));
    }
}

#[test]
fn kubernetes_samples_need_explanations_and_only_enroll_after_the_last_step() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    app.submit("/topic k8s-kubectl-basics").unwrap();
    let steps = app.lesson().teaching.as_ref().unwrap().steps.clone();
    for (index, step) in steps.iter().enumerate() {
        app.submit(&step.commands[0]).unwrap();
        assert!(app.output.contains("no cluster connection"));
        assert!(!app.progress.ready);
        assert!(app.progress.learned_at.is_none());
        app.queue.clear();
        app.submit("The output shows the information needed for this step, and I can explain what it means.").unwrap();
        let turn = app.queue.pop_back().unwrap();
        app.apply_reply(
            TeachingReply {
                message: "That explains the observation.".into(),
                assessment: Assessment::Understood,
                evidence: step.goal.clone(),
                practice_offered: false,
                review_prompt_for: None,
                review_result: None,
            },
            &turn,
        )
        .unwrap();
        assert!(app.progress.ready);
        assert_eq!(app.progress.learned_at.is_some(), index + 1 == steps.len());
        if index + 1 < steps.len() {
            app.submit("/next").unwrap();
        }
        app.queue.clear();
    }
    app.store
        .db
        .execute("UPDATE reviews SET due_at='2000-01-01T00:00:00Z'", [])
        .unwrap();
    app.submit("/level 1").unwrap();
    let due = app.store.teaching_due().unwrap();
    assert_eq!(due[0].topic_id, "k8s-kubectl-basics");
}

#[test]
fn model_claims_cannot_replace_real_command_practice() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    app.submit("/topic linux-grep-1").unwrap();
    app.progress.practice_offered = true;
    app.queue.clear();
    app.submit("I can explain the command but have not run it yet.")
        .unwrap();
    let turn = app.queue.pop_back().unwrap();
    app.apply_reply(
        TeachingReply {
            message: "Try it in the lab.".into(),
            assessment: Assessment::Understood,
            evidence: "A text explanation".into(),
            practice_offered: false,
            review_prompt_for: None,
            review_result: None,
        },
        &turn,
    )
    .unwrap();
    assert!(!app.progress.ready);
    assert!(app.progress.learned_at.is_none());
}

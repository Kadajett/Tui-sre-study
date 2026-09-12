use crate::{
    teaching::{Teacher, Turn, TurnKind},
    teaching_protocol::{Assessment, TeachingReply},
    Store,
};
use tempfile::TempDir;

fn app(dir: &TempDir) -> Teacher {
    let lessons = serde_json::from_str(include_str!("../data/lessons.json")).unwrap();
    let mut app = Teacher::new(
        Store::open(&dir.path().join("progress.db")).unwrap(),
        lessons,
        Some("du"),
    )
    .unwrap();
    app.online = false;
    app.queue.clear();
    app
}

fn understood() -> TeachingReply {
    TeachingReply {
        message: "Your observations explain the service path.".into(),
        assessment: Assessment::Understood,
        evidence: "Cited real evidence and verification".into(),
        practice_offered: false,
        review_prompt_for: None,
        review_result: None,
    }
}

#[test]
fn incident_unlocks_only_after_all_prerequisites_are_learned() {
    for scenario in crate::scenario_catalog::catalog() {
        let dir = TempDir::new().unwrap();
        let mut app = app(&dir);
        assert!(app.start_scenario(&scenario.id).is_err());
        assert!(app.lab.active.is_none());
        for id in &scenario.prerequisites {
            assert!(
                app.catalog.iter().any(|lesson| &lesson.id == id),
                "Missing prerequisite {id}"
            );
            app.store
                .learn_topic(id, &Default::default(), "Test prerequisite evidence")
                .unwrap();
        }
        assert!(crate::scenario_catalog::missing(&app.store, &scenario)
            .unwrap()
            .is_empty());
    }
}

#[test]
fn a_model_claim_cannot_override_failed_recovery_or_complete_normal_teaching() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    app.lab.active = Some("k8s-selector".into());
    app.lab.scenario = true;
    let mut turn=Turn {text:"The selector mismatched the actual labels; I corrected it and verified an HTTP response.".into(),kind:TurnKind::Scenario {id:"k8s-selector".into(),evaluate:true,verified:false},can_assess:true,review_for:None,output:String::new(),lab_topic:None};
    app.apply_reply(understood(), &turn).unwrap();
    assert!(!app.lab.completed);
    assert!(!app.progress.ready);
    assert!(app.progress.learned_at.is_none());
    turn.kind = TurnKind::Scenario {
        id: "k8s-selector".into(),
        evaluate: true,
        verified: true,
    };
    app.apply_reply(understood(), &turn).unwrap();
    assert!(app.lab.completed);
    assert!(!app.progress.ready);
    assert_eq!(app.store.stats().0, 0);
}

#[test]
fn lab_state_and_interrupted_operation_resume_without_executing_again() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    app.progress.step = 2;
    app.store
        .save_topic(&app.lesson().id, &app.progress)
        .unwrap();
    app.lab.active = Some("docker-network".into());
    app.lab.scenario = true;
    app.lab.pending = Some("command docker network connect".into());
    app.lab.evidence.push(crate::teaching_scenarios::Evidence {
        command: "docker inspect sre-practice-web".into(),
        success: true,
        output: "saved network evidence".into(),
    });
    app.checkpoint().unwrap();
    drop(app);
    let resumed = self::app(&dir);
    assert_eq!(resumed.progress.step, 2);
    assert_eq!(resumed.lab.active.as_deref(), Some("docker-network"));
    assert_eq!(resumed.lab.evidence[0].output, "saved network evidence");
    assert!(resumed.lab.pending.is_none());
    assert!(resumed.lab_worker.is_none());
    assert!(resumed
        .transcript
        .last()
        .unwrap()
        .contains("will not run again automatically"));
}

#[test]
fn every_fixture_is_bounded_and_does_not_mount_credentials() {
    for id in [
        "k8s-basics",
        "k8s-selector",
        "k8s-readiness",
        "k8s-crash",
        "k8s-pending",
    ] {
        let manifest = crate::lab_kubernetes::manifest(id);
        let spec = &manifest["items"][0]["spec"]["template"]["spec"];
        assert_eq!(spec["automountServiceAccountToken"], false);
        assert_eq!(
            spec["containers"][0]["securityContext"]["runAsNonRoot"],
            true
        );
        assert_eq!(
            spec["containers"][0]["resources"]["limits"]["memory"],
            "64Mi"
        );
        assert!(spec.get("hostNetwork").is_none());
        assert!(spec["volumes"][0].get("hostPath").is_none());
    }
}

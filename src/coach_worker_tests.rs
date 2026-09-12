use super::*;

fn pending(budget: Budget) -> (Coach, mpsc::Sender<Reply>) {
    let (sender, receiver) = mpsc::channel();
    (
        Coach {
            pending: Some(Pending {
                receiver,
                budget,
                generation: 7,
                prompt: "saved turn".into(),
            }),
        },
        sender,
    )
}

#[test]
fn disconnected_worker_reports_error_to_the_current_generation() {
    let (mut coach, sender) = pending(Budget::default());
    drop(sender);
    let reply = coach.poll().unwrap();
    assert_eq!(reply.generation, 7);
    assert_eq!(reply.prompt, "saved turn");
    assert!(reply
        .text
        .unwrap_err()
        .to_string()
        .contains("worker stopped"));
    assert!(!coach.busy());
}

#[test]
fn expired_worker_releases_thinking_and_discards_late_results() {
    let (mut coach, sender) = pending(Budget::expired());
    let reply = coach.poll().unwrap();
    assert_eq!(reply.generation, 7);
    assert!(reply
        .text
        .unwrap_err()
        .to_string()
        .contains("60-second limit"));
    assert!(!coach.busy());
    assert!(sender
        .send(Reply {
            generation: 7,
            prompt: "old".into(),
            text: Ok("late".into())
        })
        .is_err());
    assert!(coach.poll().is_none());
}

#[test]
fn active_worker_shows_progress_and_yields_one_reply() {
    let budget = Budget::default();
    let (mut coach, sender) = pending(budget.clone());
    budget.phase(2);
    assert!(coach.status().unwrap().contains("looking up references"));
    assert!(coach.poll().is_none());
    sender
        .send(Reply {
            generation: 7,
            prompt: "saved turn".into(),
            text: Ok("answer".into()),
        })
        .unwrap();
    assert_eq!(coach.poll().unwrap().text.unwrap(), "answer");
    assert!(!coach.busy());
    assert!(coach.status().is_none());
}

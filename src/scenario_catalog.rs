use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Scenario {
    pub id: String,
    pub title: String,
    pub prerequisites: Vec<String>,
    pub brief: String,
    pub objective: String,
}

pub fn catalog() -> Vec<Scenario> {
    serde_json::from_str(include_str!("../data/scenarios.json")).expect("bundled scenario catalog")
}

pub fn known(lab: &str) -> bool {
    matches!(lab, "k8s-basics" | "docker-basics") || catalog().iter().any(|s| s.id == lab)
}

pub fn missing(store: &crate::Store, scenario: &Scenario) -> anyhow::Result<Vec<String>> {
    scenario
        .prerequisites
        .iter()
        .filter_map(|id| match store.topic_progress(id) {
            Ok(progress) if progress.learned_at.is_some() => None,
            Ok(_) => Some(Ok(id.clone())),
            Err(error) => Some(Err(error)),
        })
        .collect()
}

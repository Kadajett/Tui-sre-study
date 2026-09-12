use crate::{
    lab_runtime::{Request, Response},
    teaching::{Teacher, Turn, TurnKind},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, Receiver};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LabState {
    pub active: Option<String>,
    #[serde(default)]
    pub started_at: String,
    pub scenario: bool,
    pub completed: bool,
    pub evidence: Vec<Evidence>,
    pub pending: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub command: String,
    pub success: bool,
    pub output: String,
}

pub struct Job {
    request: Request,
    explanation: String,
    result: Result<Response>,
}

impl Teacher {
    pub(super) fn scenarios(&mut self) -> Result<()> {
        let mut lines = vec!["Scenarios unlock after their prerequisites are learned. /scenario ID starts a real incident; /solve YOUR EXPLANATION requests an evidence-based assessment.".into()];
        for scenario in crate::scenario_catalog::catalog() {
            let missing = crate::scenario_catalog::missing(&self.store, &scenario)?;
            let status = if missing.is_empty() {
                "Available".into()
            } else {
                format!("Learn first: {}", missing.join(", "))
            };
            lines.push(format!(
                "**{}** — `{}`\n{}",
                scenario.title, scenario.id, status
            ));
        }
        self.say("Incident practice", &lines.join("\n\n"));
        Ok(())
    }

    pub(super) fn start_scenario(&mut self, id: &str) -> Result<()> {
        let scenario = crate::scenario_catalog::catalog()
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| anyhow::anyhow!("Unknown scenario. Use /scenarios"))?;
        let missing = crate::scenario_catalog::missing(&self.store, &scenario)?;
        ensure!(
            missing.is_empty(),
            "This scenario follows teaching. Learn these topics first: {}",
            missing.join(", ")
        );
        self.start_lab(id, true)?;
        self.touch_review_session(chrono::Utc::now())?;
        self.generation += 1;
        self.queue.clear();
        self.in_flight = None;
        self.say("Incident briefing", &format!("### {}\n\n{}\n\n{}\n\nType commands here. `/lab commands` lists supported commands, `/hint` asks for coaching, and `/solve` followed by your explanation checks recovery. Your normal lesson stays saved.", scenario.title, scenario.brief, scenario.objective));
        Ok(())
    }

    pub(super) fn start_lab(&mut self, id: &str, scenario: bool) -> Result<()> {
        ensure!(
            self.lab.active.as_deref().is_none_or(|active| active == id),
            "A practice lab is already active. Use /lab stop before changing it."
        );
        ensure!(
            self.lab.pending.is_none(),
            "A real command is still running"
        );
        if self.lab.active.is_none() {
            self.lab = LabState {
                active: Some(id.into()),
                started_at: chrono::Utc::now().to_rfc3339(),
                scenario,
                ..LabState::default()
            };
        }
        self.lab_job("start", "", "")
    }

    pub(super) fn lab_control(&mut self, argument: &str) -> Result<()> {
        match argument {
            "kubernetes" => self.start_lab("k8s-basics", false),
            "docker" => self.start_lab("docker-basics", false),
            "stop" => self.lab_job("stop", "", ""),
            "commands" => {
                let id = self.lab.active.as_deref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Start /lab kubernetes or /lab docker, or an unlocked /scenario ID"
                    )
                })?;
                let commands = crate::lab_commands::available(id);
                self.say(
                    "Real lab commands",
                    &commands
                        .iter()
                        .map(|c| format!("```bash\n{c}\n```"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
                Ok(())
            }
            _ => {
                self.say("Practice lab", "`/lab kubernetes` or `/lab docker` starts a healthy teaching environment. `/lab commands` lists real supported commands. `/lab stop` removes only practice resources. To reset, stop then explicitly start the lab again. A restart preserves the active lab.");
                Ok(())
            }
        }
    }

    pub(super) fn solve_scenario(&mut self, explanation: &str) -> Result<()> {
        ensure!(
            self.lab.scenario && !self.lab.completed,
            "Start an unlocked incident with /scenario ID first"
        );
        ensure!(
            self.lab.evidence.len() >= 2,
            "Collect command evidence before submitting a diagnosis"
        );
        ensure!(explanation.len()>=40 && crate::teaching_protocol::substantive(explanation), "Explain the failure, your evidence, the correction, and how you checked recovery after /solve");
        self.lab_job("verify", "", explanation)
    }

    pub(super) fn lab_job(&mut self, action: &str, command: &str, explanation: &str) -> Result<()> {
        ensure!(
            self.lab.pending.is_none(),
            "Wait for the running lab operation to finish"
        );
        let id = self
            .lab
            .active
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Start /lab kubernetes or /lab docker first"))?;
        if action == "command" {
            crate::lab_commands::validate(&id, command)?;
        }
        let request = Request {
            action: action.into(),
            lab: id,
            command: command.into(),
        };
        self.lab.pending = Some(format!("{action} {command}"));
        self.checkpoint()?;
        let (sender, receiver) = mpsc::channel();
        let explanation = explanation.to_owned();
        std::thread::spawn(move || {
            let result = crate::lab_runtime::remote(&request);
            let _ = sender.send(Job {
                request,
                explanation,
                result,
            });
        });
        self.lab_worker = Some(receiver);
        self.say(
            "Practice runner",
            &format!("Running `{action} {command}` against real practice resources…"),
        );
        Ok(())
    }

    pub(super) fn poll_lab(&mut self) -> Result<()> {
        let Some(worker) = self.lab_worker.as_ref() else {
            return Ok(());
        };
        let job = match worker.try_recv() {
            Ok(job) => job,
            Err(mpsc::TryRecvError::Empty) => return Ok(()),
            Err(error) => {
                self.lab_worker = None;
                self.lab.pending = None;
                return Err(error.into());
            }
        };
        self.lab_worker = None;
        self.lab.pending = None;
        self.finish_lab(job)?;
        self.archive_scenario()?;
        self.checkpoint()
    }

    fn finish_lab(&mut self, job: Job) -> Result<()> {
        let response = match job.result {
            Ok(response) => response,
            Err(error) => {
                self.say("Practice runner", &format!("{error}. The command has not been retried. Inspect the lab before trying again."));
                return Ok(());
            }
        };
        self.output = format!(
            "REAL PRACTICE LAB • {}\n$ {}\n{}\nExit success: {}",
            job.request.lab, job.request.command, response.output, response.ok
        );
        self.view.output.home();
        match job.request.action.as_str() {
            "command" => self.record_lab_command(&job.request, response),
            "verify" => self.scenario_turn(&job.explanation, Some(response.ok), &response.output),
            "start" if !response.ok => {
                self.archive_scenario()?;
                self.lab = LabState::default();
                self.say("Practice runner", &response.output);
                self.say("Practice runner", "The lab did not start successfully. If another lab is reported active, start that named lab to resume it, then stop it explicitly before switching.");
                Ok(())
            }
            "stop" if response.ok => {
                if self.lab.scenario {
                    self.generation += 1;
                    self.queue.clear();
                    self.in_flight = None;
                }
                self.archive_scenario()?;
                self.lab = LabState::default();
                self.say("Practice runner", &response.output);
                Ok(())
            }
            _ => {
                self.say("Practice runner", &response.output);
                Ok(())
            }
        }
    }

    fn record_lab_command(&mut self, request: &Request, response: Response) -> Result<()> {
        self.lab.evidence.push(Evidence {
            command: request.command.clone(),
            success: response.ok,
            output: response.output.clone(),
        });
        if self.lab.evidence.len() > 24 {
            self.lab.evidence.remove(0);
        }
        if self.lab.scenario {
            return self.scenario_turn(&request.command, None, &response.output);
        }
        let matches_step = crate::teaching_curriculum::step(self.lesson(), self.progress.step)
            .is_some_and(|step| {
                step.commands.iter().any(|command| {
                    shell_words::split(command).ok() == shell_words::split(&request.command).ok()
                })
            });
        if response.ok && matches_step {
            self.lab_practiced = Some((self.lesson().id.clone(), self.progress.step));
        }
        self.progress.practice_offered |= response.ok && matches_step;
        self.store.save_topic(&self.lesson().id, &self.progress)?;
        self.enqueue(&request.command, TurnKind::Lab(response.ok))
    }

    pub(super) fn scenario_turn(
        &mut self,
        text: &str,
        assessment: Option<bool>,
        check: &str,
    ) -> Result<()> {
        ensure!(
            self.queue.len() < 8,
            "Let the teacher catch up before sending another message"
        );
        let id = self
            .lab
            .active
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active incident"))?;
        self.say("You", text);
        self.queue.push_back(Turn {
            text: text.into(),
            kind: TurnKind::Scenario {
                id,
                evaluate: assessment.is_some(),
                verified: assessment.unwrap_or(false),
            },
            can_assess: false,
            review_for: None,
            output: serde_json::json!({"commands":self.lab.evidence.iter().rev().take(12).map(|item| serde_json::json!({"command":item.command,"success":item.success,"output":item.output.chars().take(4000).collect::<String>()})).collect::<Vec<_>>(),"recovery_check":check})
                .to_string(),
            lab_topic: None,
        });
        Ok(())
    }
}

pub type Worker = Receiver<Job>;

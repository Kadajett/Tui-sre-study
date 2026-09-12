use anyhow::{ensure, Result};
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(super) struct Budget {
    started: Instant,
    limit: Duration,
    phase: Arc<AtomicU8>,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            limit: Duration::from_secs(60),
            phase: Arc::new(AtomicU8::new(0)),
        }
    }
}

impl Budget {
    #[cfg(test)]
    pub(super) fn expired() -> Self {
        Self {
            started: Instant::now() - Duration::from_secs(61),
            ..Self::default()
        }
    }

    pub(super) fn remaining(&self) -> Result<Duration> {
        let left = self.limit.saturating_sub(self.started.elapsed());
        ensure!(
            !left.is_zero(),
            "Mercury took too long (60-second limit). Please try your message again"
        );
        Ok(left)
    }

    pub(super) fn phase(&self, phase: u8) {
        self.phase.store(phase, Ordering::Relaxed);
    }

    pub(super) fn status(&self) -> String {
        let phase = match self.phase.load(Ordering::Relaxed) {
            1 => "retrying empty/invalid reply",
            2 => "looking up references",
            _ => "waiting for reply",
        };
        format!("{phase} · {}s / 60s", self.started.elapsed().as_secs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_phase_and_expired_budget_cannot_restart() {
        let budget = Budget {
            started: Instant::now() - Duration::from_secs(61),
            ..Budget::default()
        };
        let worker = budget.clone();
        worker.phase(1);
        assert!(budget.status().contains("retrying"));
        assert!(worker.remaining().is_err());
        assert!(budget.remaining().is_err());
    }
}

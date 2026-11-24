//! Fleet-wide concurrency caps: how many runs may be live at once across
//! every project and agent, by kind.
//!
//! `max_concurrent` on a manifest caps one agent. These cap the machine.
//! Model runs (claude, codex) are the expensive kind — each holds a model
//! session, a rate-limit share and real money — so they get a small cap.
//! Script runs are cheap shell stages that mostly fan out and wait, so they
//! get a large one. A run over its cap is not failed: it stays `Pending`
//! and the next spawn pass starts it once a slot frees, oldest first.
//!
//! Fixed on purpose. A cap that a manifest or a flag can raise is a cap a
//! runaway chain can raise.

use agent_core::types::Runner;

/// Model runs (claude + codex) live at once.
pub(crate) const MAX_MODEL_RUNS: u32 = 5;
/// Script runs live at once.
pub(crate) const MAX_SCRIPT_RUNS: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Model,
    Script,
}

impl Kind {
    pub(crate) fn of(runner: Runner) -> Self {
        match runner {
            Runner::Claude | Runner::Codex => Self::Model,
            Runner::Script => Self::Script,
        }
    }

    pub(crate) fn cap(self) -> u32 {
        match self {
            Self::Model => MAX_MODEL_RUNS,
            Self::Script => MAX_SCRIPT_RUNS,
        }
    }
}

/// Live runs by kind, for one spawn pass.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Slots {
    model: u32,
    script: u32,
}

impl Slots {
    fn used(self, kind: Kind) -> u32 {
        match kind {
            Kind::Model => self.model,
            Kind::Script => self.script,
        }
    }

    pub(crate) fn has_room(self, kind: Kind) -> bool {
        self.used(kind) < kind.cap()
    }

    pub(crate) fn take(&mut self, kind: Kind) {
        match kind {
            Kind::Model => self.model += 1,
            Kind::Script => self.script += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_and_codex_share_the_model_cap() {
        assert_eq!(Kind::of(Runner::Claude), Kind::Model);
        assert_eq!(Kind::of(Runner::Codex), Kind::Model);
        assert_eq!(Kind::of(Runner::Script), Kind::Script);
    }

    #[test]
    fn the_sixth_model_run_waits() {
        let mut slots = Slots::default();
        for _ in 0..MAX_MODEL_RUNS {
            assert!(slots.has_room(Kind::Model));
            slots.take(Kind::Model);
        }
        assert!(!slots.has_room(Kind::Model));
    }

    #[test]
    fn a_full_model_cap_does_not_block_scripts() {
        let mut slots = Slots::default();
        for _ in 0..MAX_MODEL_RUNS {
            slots.take(Kind::Model);
        }
        assert!(slots.has_room(Kind::Script));
        for _ in 0..MAX_SCRIPT_RUNS {
            slots.take(Kind::Script);
        }
        assert!(!slots.has_room(Kind::Script));
    }

    #[test]
    fn the_caps_are_five_and_twenty() {
        assert_eq!(MAX_MODEL_RUNS, 5);
        assert_eq!(MAX_SCRIPT_RUNS, 20);
    }
}

//! Startup routing for the native workspace.
//!
//! Adapted from T3 Code v0.1.11, apps/web/src/onboarding/firstRun.logic.ts.
//! Copyright (c) 2026 T3 Tools Inc. See LICENSES/T3-Code-MIT.txt.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Route {
    #[default]
    Pending,
    Workspace,
    Welcome,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gate {
    pub route: Route,
    pub stalled: bool,
}

impl Gate {
    pub fn observe(&mut self, route: Route) {
        if route != Route::Pending && self.route != route {
            *self = Self {
                route,
                stalled: false,
            };
        }
    }

    pub fn timeout(&mut self) {
        if self.route == Route::Pending {
            self.stalled = true;
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StartupEvidence {
    pub enabled: bool,
    pub hydrated: bool,
    pub previously_completed: bool,
    pub bootstrapped: bool,
    pub authoritative: bool,
    pub workspace_authoritative: bool,
    pub provenance_authoritative: bool,
    pub catalog_ready: bool,
    pub server_config_available: bool,
    pub fresh_workspace: bool,
    pub project_count: usize,
    pub thread_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupOutcome {
    pub route: Route,
    pub persist_completion: bool,
}

impl StartupEvidence {
    pub fn resolve(self) -> StartupOutcome {
        let outcome = |route, persist_completion| StartupOutcome {
            route,
            persist_completion,
        };
        if !self.enabled || (self.hydrated && self.previously_completed) {
            return outcome(Route::Workspace, false);
        }
        if !self.hydrated {
            return outcome(Route::Pending, false);
        }

        let ready = self.bootstrapped
            && self.authoritative
            && self.catalog_ready
            && self.server_config_available;
        if self.project_count > 1 || self.thread_count > 1 {
            return outcome(Route::Workspace, ready && self.workspace_authoritative);
        }
        if !ready || !self.provenance_authoritative {
            return outcome(Route::Pending, false);
        }
        if self.fresh_workspace {
            return outcome(Route::Welcome, false);
        }
        outcome(Route::Workspace, self.workspace_authoritative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> StartupEvidence {
        StartupEvidence {
            enabled: true,
            hydrated: true,
            bootstrapped: true,
            authoritative: true,
            workspace_authoritative: true,
            provenance_authoritative: true,
            catalog_ready: true,
            server_config_available: true,
            fresh_workspace: true,
            ..Default::default()
        }
    }

    #[test]
    fn disabled_or_completed_skips_welcome_without_persisting_again() {
        let expected = StartupOutcome {
            route: Route::Workspace,
            persist_completion: false,
        };
        assert_eq!(StartupEvidence::default().resolve(), expected);
        assert_eq!(
            StartupEvidence {
                previously_completed: true,
                ..fresh()
            }
            .resolve(),
            expected
        );
    }

    #[test]
    fn fresh_profile_waits_for_hydration_catalog_and_provenance() {
        for input in [
            StartupEvidence {
                hydrated: false,
                ..fresh()
            },
            StartupEvidence {
                catalog_ready: false,
                ..fresh()
            },
            StartupEvidence {
                server_config_available: false,
                ..fresh()
            },
            StartupEvidence {
                provenance_authoritative: false,
                ..fresh()
            },
        ] {
            assert_eq!(
                input.resolve(),
                StartupOutcome {
                    route: Route::Pending,
                    persist_completion: false
                }
            );
        }
        assert_eq!(
            fresh().resolve(),
            StartupOutcome {
                route: Route::Welcome,
                persist_completion: false
            }
        );
    }

    #[test]
    fn existing_workspace_can_open_before_it_is_safe_to_mark_setup_complete() {
        let existing = StartupEvidence {
            thread_count: 2,
            ..fresh()
        };
        assert_eq!(
            StartupEvidence {
                workspace_authoritative: false,
                ..existing
            }
            .resolve(),
            StartupOutcome {
                route: Route::Workspace,
                persist_completion: false
            }
        );
        assert_eq!(
            existing.resolve(),
            StartupOutcome {
                route: Route::Workspace,
                persist_completion: true
            }
        );
        assert_eq!(
            StartupEvidence {
                fresh_workspace: false,
                ..fresh()
            }
            .resolve(),
            StartupOutcome {
                route: Route::Workspace,
                persist_completion: true
            }
        );
    }

    #[test]
    fn late_evidence_recovers_a_stalled_gate_but_timeout_cannot_replace_a_screen() {
        let mut gate = Gate::default();
        gate.timeout();
        gate.observe(Route::Pending);
        assert_eq!(
            gate,
            Gate {
                route: Route::Pending,
                stalled: true
            }
        );
        gate.observe(Route::Welcome);
        gate.timeout();
        assert_eq!(
            gate,
            Gate {
                route: Route::Welcome,
                stalled: false
            }
        );
        gate.observe(Route::Workspace);
        assert_eq!(
            gate,
            Gate {
                route: Route::Workspace,
                stalled: false
            }
        );
    }
}

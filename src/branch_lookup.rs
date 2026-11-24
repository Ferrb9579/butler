//! One bounded branch lookup at a time for the currently visible details row.
use butler::threads::{Conversation, Workspace};
use incular::{prelude::Signal, runtime::BuildContext};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};
use uuid::Uuid;

pub fn visible_target<'a>(
    selected: Option<&'a Conversation>,
    registered: Option<&Path>,
    details_open: bool,
) -> Option<(Uuid, &'a Path)> {
    if !details_open {
        return None;
    }
    let selected = selected?;
    let Workspace::Local(root) = &selected.workspace else {
        return None;
    };
    (registered == Some(root.as_path())).then_some((selected.id, root.as_path()))
}

type Target = (Uuid, PathBuf);

#[derive(Default)]
struct State {
    target: Option<Target>,
    generation: u64,
    busy: bool,
    settled: bool,
    branch: Option<String>,
}

impl State {
    fn select(&mut self, target: Option<Target>) {
        if self.target != target {
            self.target = target;
            self.generation = self.generation.wrapping_add(1);
            self.branch = None;
            self.settled = false;
        }
    }

    fn complete(&mut self, generation: u64, result: Option<String>) {
        self.busy = false;
        if self.generation == generation {
            self.branch = result;
            self.settled = true;
        }
    }
}

#[derive(Clone)]
pub struct Lookup {
    state: Rc<RefCell<State>>,
    revision: Signal<u64>,
    probe: fn(&Path) -> Option<String>,
}

impl Default for Lookup {
    fn default() -> Self {
        Self {
            state: Rc::default(),
            revision: Signal::new(0),
            probe: butler::workspace_branch::named,
        }
    }
}

impl Lookup {
    #[cfg(test)]
    pub fn with_probe(probe: fn(&Path) -> Option<String>) -> Self {
        Self {
            probe,
            ..Self::default()
        }
    }

    pub fn poll(&self, cx: &BuildContext, target: Option<(Uuid, &Path)>) -> Option<String> {
        self.revision.get();
        let mut state = self.state.borrow_mut();
        state.select(target.map(|(id, path)| (id, path.to_owned())));
        if !state.busy
            && !state.settled
            && let Some((_, root)) = &state.target
        {
            let root = root.clone();
            let generation = state.generation;
            let next = self.clone();
            let probe = self.probe;
            state.busy = true;
            let _ = cx.spawn_blocking(
                move || probe(&root),
                move |result, _| {
                    next.state
                        .borrow_mut()
                        .complete(generation, result.ok().flatten());
                    next.revision.set(next.revision.get().wrapping_add(1));
                },
            );
        }
        state.branch.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_open_saved_local_thread_in_the_registered_project_can_probe() {
        let registered = Path::new("/project");
        let mut thread = Conversation {
            id: Uuid::new_v4(),
            workspace: Workspace::Scratch,
            prompt: String::new(),
            reply: String::new(),
            follow_ups: Vec::new(),
        };
        assert_eq!(visible_target(Some(&thread), Some(registered), true), None);
        thread.workspace = Workspace::Local(registered.to_owned());
        assert_eq!(visible_target(Some(&thread), Some(registered), false), None);
        assert_eq!(visible_target(Some(&thread), None, true), None);
        assert_eq!(
            visible_target(Some(&thread), Some(Path::new("/another")), true),
            None
        );
        assert_eq!(
            visible_target(Some(&thread), Some(registered), true),
            Some((thread.id, registered))
        );
    }

    #[test]
    fn a_stale_completion_cannot_cross_selections_even_when_returning_to_the_same_thread() {
        let mut state = State::default();
        let first = Some((Uuid::new_v4(), "/first".into()));
        let second = Some((Uuid::new_v4(), "/second".into()));
        state.select(first.clone());
        let previous = state.generation;
        state.busy = true;
        state.select(second);
        state.select(None);
        state.select(first);
        assert!(state.busy);
        state.complete(previous, Some("old".into()));
        assert!(!state.busy);
        assert!(!state.settled);
        assert!(state.branch.is_none());
        state.complete(state.generation, Some("current".into()));
        assert_eq!(state.branch.as_deref(), Some("current"));
        state.select(None);
        assert!(state.branch.is_none());
    }
}

//! Index routing after setup. Reference: T3 Code v0.1.11,
//! apps/web/src/routes/_chat.index.tsx and components/NoProjectsHero.tsx.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Landing<'a> {
    /// No project has loaded yet; an unbootstrapped environment isn't an empty workspace.
    Loading,
    Empty {
        can_start_without_project: bool,
    },
    /// Open a draft in the most recent available project.
    Project {
        id: &'a str,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Project<'a> {
    pub id: &'a str,
    pub archived: bool,
}

/// `projects` is in workspace activity order (most recent first). The workspace
/// store, not the view, is responsible for ordering visible projects and threads.
pub fn resolve_landing<'a>(
    projects: &[Project<'a>],
    bootstrapped: bool,
    scratch_available: bool,
) -> Landing<'a> {
    if let Some(project) = projects.iter().find(|project| !project.archived) {
        Landing::Project { id: project.id }
    } else if bootstrapped {
        Landing::Empty {
            can_start_without_project: scratch_available,
        }
    } else {
        Landing::Loading
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_bootstrap_before_claiming_the_workspace_is_empty() {
        assert_eq!(resolve_landing(&[], false, true), Landing::Loading);
        assert_eq!(
            resolve_landing(&[], true, true),
            Landing::Empty {
                can_start_without_project: true
            }
        );
    }

    #[test]
    fn skips_archived_projects_even_if_they_are_more_recent() {
        let projects = [
            Project {
                id: "archive",
                archived: true,
            },
            Project {
                id: "recent",
                archived: false,
            },
            Project {
                id: "older",
                archived: false,
            },
        ];
        assert_eq!(
            resolve_landing(&projects, false, false),
            Landing::Project { id: "recent" }
        );
    }

    #[test]
    fn archived_only_is_empty_and_scratch_requires_capability() {
        let projects = [Project {
            id: "archive",
            archived: true,
        }];
        assert_eq!(
            resolve_landing(&projects, true, false),
            Landing::Empty {
                can_start_without_project: false
            }
        );
    }
}

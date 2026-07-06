use serde::{Deserialize, Serialize};

use crate::db::ProjectStore;
use crate::error::GaplyError;

pub const NAME_MAX_LEN: usize = 120;
pub const DESCRIPTION_MAX_LEN: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// Unix epoch seconds.
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub description: String,
    pub created_at: i64,
}

impl NewProject {
    pub fn now(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            created_at: crate::now_epoch(),
        }
    }
}

/// Validate and create a project. Names are trimmed; empty or oversized
/// input is rejected before touching the store.
#[tracing::instrument(skip(store), fields(name = %name))]
pub fn create_project(
    store: &dyn ProjectStore,
    name: &str,
    description: &str,
) -> Result<Project, GaplyError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(GaplyError::Validation("project name must not be empty".into()));
    }
    if name.chars().count() > NAME_MAX_LEN {
        return Err(GaplyError::Validation(format!(
            "project name must be at most {NAME_MAX_LEN} characters"
        )));
    }
    let description = description.trim();
    if description.chars().count() > DESCRIPTION_MAX_LEN {
        return Err(GaplyError::Validation(format!(
            "description must be at most {DESCRIPTION_MAX_LEN} characters"
        )));
    }

    let created = store.insert_project(&NewProject::now(name, description))?;
    tracing::info!(id = created.id, "project created");
    Ok(created)
}

#[tracing::instrument(skip(store))]
pub fn list_projects(store: &dyn ProjectStore) -> Result<Vec<Project>, GaplyError> {
    store.list_projects()
}

/// Fetch one project; absence is a typed `NotFound` error, not an Option,
/// so shells can surface it uniformly.
#[tracing::instrument(skip(store))]
pub fn get_project(store: &dyn ProjectStore, id: i64) -> Result<Project, GaplyError> {
    store
        .get_project(id)?
        .ok_or(GaplyError::NotFound { entity: "project", id: id.to_string() })
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use super::*;

    /// In-memory mock — no SQLite involved.
    #[derive(Default)]
    pub struct MockStore {
        pub projects: Mutex<Vec<Project>>,
        pub fail_with: Mutex<Option<String>>,
    }

    impl ProjectStore for MockStore {
        fn insert_project(&self, new: &NewProject) -> Result<Project, GaplyError> {
            if let Some(msg) = self.fail_with.lock().unwrap().as_ref() {
                return Err(GaplyError::Database(msg.clone()));
            }
            let mut projects = self.projects.lock().unwrap();
            if projects.iter().any(|p| p.name == new.name) {
                return Err(GaplyError::Conflict(format!(
                    "a project named \"{}\" already exists",
                    new.name
                )));
            }
            let project = Project {
                id: projects.len() as i64 + 1,
                name: new.name.clone(),
                description: new.description.clone(),
                created_at: new.created_at,
            };
            projects.push(project.clone());
            Ok(project)
        }

        fn list_projects(&self) -> Result<Vec<Project>, GaplyError> {
            Ok(self.projects.lock().unwrap().clone())
        }

        fn get_project(&self, id: i64) -> Result<Option<Project>, GaplyError> {
            Ok(self.projects.lock().unwrap().iter().find(|p| p.id == id).cloned())
        }

        fn ping(&self) -> Result<(), GaplyError> {
            Ok(())
        }
    }

    #[test]
    fn create_trims_and_stores() {
        let store = MockStore::default();
        let p = create_project(&store, "  Thesis Pipeline  ", "  drafts  ").unwrap();
        assert_eq!(p.name, "Thesis Pipeline");
        assert_eq!(p.description, "drafts");
        assert!(p.created_at > 0);
    }

    #[test]
    fn empty_name_rejected_before_store() {
        let store = MockStore::default();
        let err = create_project(&store, "   ", "x").unwrap_err();
        assert!(matches!(err, GaplyError::Validation(_)));
        assert!(store.projects.lock().unwrap().is_empty());
    }

    #[test]
    fn oversized_name_rejected() {
        let store = MockStore::default();
        let long = "x".repeat(NAME_MAX_LEN + 1);
        assert!(matches!(
            create_project(&store, &long, "").unwrap_err(),
            GaplyError::Validation(_)
        ));
    }

    #[test]
    fn oversized_description_rejected() {
        let store = MockStore::default();
        let long = "d".repeat(DESCRIPTION_MAX_LEN + 1);
        assert!(matches!(
            create_project(&store, "ok", &long).unwrap_err(),
            GaplyError::Validation(_)
        ));
    }

    #[test]
    fn duplicate_name_surfaces_conflict() {
        let store = MockStore::default();
        create_project(&store, "Same", "").unwrap();
        assert!(matches!(
            create_project(&store, "Same", "").unwrap_err(),
            GaplyError::Conflict(_)
        ));
    }

    #[test]
    fn get_missing_is_not_found() {
        let store = MockStore::default();
        let err = get_project(&store, 7).unwrap_err();
        assert!(matches!(err, GaplyError::NotFound { entity: "project", .. }));
    }

    #[test]
    fn store_failure_propagates() {
        let store = MockStore::default();
        *store.fail_with.lock().unwrap() = Some("disk on fire".into());
        assert!(matches!(
            create_project(&store, "ok", "").unwrap_err(),
            GaplyError::Database(_)
        ));
    }

    #[test]
    fn list_returns_all() {
        let store = MockStore::default();
        create_project(&store, "One", "").unwrap();
        create_project(&store, "Two", "").unwrap();
        assert_eq!(list_projects(&store).unwrap().len(), 2);
    }
}

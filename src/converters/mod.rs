use crate::converters::pyproject_updater::PyprojectUpdater;
use crate::errors::{MIGRATION_ERRORS, MigrationError};
use crate::schema::pep_621::Project;
use crate::schema::pyproject::DependencyGroupSpecification;
use crate::schema::utils::SingleOrVec;
use crate::uv;
use crate::uv::LockType;
use indexmap::IndexMap;
use log::{error, info, warn};
use owo_colors::OwoColorize;
use std::any::Any;
use std::fmt::Debug;
use std::fs::{File, remove_file};
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::{format, fs};
use toml_edit::DocumentMut;

pub mod pip;
pub mod pipenv;
pub mod poetry;
mod pyproject_updater;

type DependencyGroupsAndDefaultGroups = (
    Option<IndexMap<String, Vec<DependencyGroupSpecification>>>,
    Option<SingleOrVec<String>>,
);

// Default project name to use in `[project.name]` when it couldn't be determined.
const DEFAULT_PROJECT_NAME: &str = "project";

#[derive(Debug, PartialEq, Eq, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ConverterOptions {
    pub project_path: PathBuf,
    pub dry_run: bool,
    pub skip_lock: bool,
    pub skip_uv_checks: bool,
    pub ignore_locked_versions: bool,
    pub replace_project_section: bool,
    pub keep_current_build_backend: bool,
    pub keep_old_metadata: bool,
    pub ignore_errors: bool,
    pub dependency_groups_strategy: Option<DependencyGroupsStrategy>,
    pub build_backend: Option<BuildBackend>,
}

/// Converts a project from a package manager to uv.
pub trait Converter: Any + Debug {
    /// Performs the conversion from the current package manager to uv.
    fn convert_to_uv(&self) {
        let pyproject_path = self.get_project_path().join("pyproject.toml");
        let had_pyproject = pyproject_path.exists();
        let old_pyproject = fs::read(&pyproject_path).ok();

        let updated_pyproject_string = self.build_uv_pyproject();

        let had_errors = self.manage_migration_errors();

        if self.is_dry_run() {
            info!(
                "{}\n{}",
                "Migrated pyproject.toml:".bold(),
                updated_pyproject_string
            );
            self.manage_migration_warnings();
            return;
        }

        let mut pyproject_file = File::create(&pyproject_path).unwrap();

        pyproject_file
            .write_all(updated_pyproject_string.as_bytes())
            .unwrap();

        // If we were not able to lock dependencies with `uv lock`, we abort the migration, and
        // either revert `pyproject.toml` file to its original content, or delete it if there was
        // none.
        if self.lock_dependencies() == Err(()) {
            self.revert_changes(had_pyproject, old_pyproject);

            error!(
                "Could not lock dependencies, aborting the migration. Consider using \"{}\" if you don't need to keep versions from the lock file, or \"{}\" if you don't want to lock dependencies at all.",
                "--ignore-locked-versions".bold(),
                "--skip-lock".bold(),
            );
            exit(1);
        }

        self.remove_constraint_dependencies(updated_pyproject_string);
        self.delete_migrated_files().unwrap();

        if had_errors {
            info!(
                "{}",
                format!(
                    "Partially migrated project from {} to uv, as errors occurred during the migration.\n",
                    self.get_package_manager_name()
                )
                .bold()
                .yellow()
            );
        } else {
            info!(
                "{}",
                format!(
                    "Successfully migrated project from {} to uv!\n",
                    self.get_package_manager_name()
                )
                .bold()
                .green()
            );
        }

        self.manage_migration_warnings();
    }

    /// Revert any change made, in case the migration is aborted after some files have already been
    /// modified.
    fn revert_changes(&self, had_pyproject: bool, old_pyproject: Option<Vec<u8>>) {
        let pyproject_path = self.get_project_path().join("pyproject.toml");

        // Some package managers do not use `pyproject.toml`, so we either revert back the content
        // of a `pyproject.toml`, or delete the file if we did not have any.
        if had_pyproject {
            let mut pyproject_file = File::create(&pyproject_path).unwrap();

            pyproject_file.write_all(&old_pyproject.unwrap()).unwrap();
        } else {
            remove_file(pyproject_path).unwrap();
        }
    }

    fn manage_migration_errors(&self) -> bool {
        let migration_errors = MIGRATION_ERRORS.lock().unwrap();
        let unrecoverable_errors: Vec<&MigrationError> =
            migration_errors.iter().filter(|e| !e.recoverable).collect();

        if unrecoverable_errors.is_empty() {
            return false;
        }

        if self.ignore_errors() {
            error!("The following errors occurred during the migration:");
        } else {
            error!(
                "Could not automatically migrate the project to uv because of the following errors:"
            );
        }

        for error in &unrecoverable_errors {
            error!("- {}", error.error);
        }

        if !self.ignore_errors() {
            exit(1);
        }

        true
    }

    fn manage_migration_warnings(&self) {
        let migration_errors = MIGRATION_ERRORS.lock().unwrap();
        let warnings: Vec<&MigrationError> =
            migration_errors.iter().filter(|e| e.recoverable).collect();

        for warning in &warnings {
            warn!("{}", warning.error);
        }
    }

    /// Build `pyproject.toml` for uv package manager based on current package manager data.
    fn build_uv_pyproject(&self) -> String;

    /// Build PEP 621 `[project]` section, keeping existing fields if the section is already
    /// defined, unless user has chosen to replace existing section.
    fn build_project(
        &self,
        current_project: Option<Project>,
        project: Project,
        version: String,
    ) -> Project {
        // "version" is required by uv, unless the attribute is set dynamically. If the user wants
        // to replace `project` section, we ignore `dynamic`, since it gets removed.
        let version = if !self.replace_project_section()
            && let Some(current_project) = current_project.as_ref()
            && current_project
                .dynamic
                .clone()
                .unwrap_or_default()
                .contains(&"version".to_string())
        {
            None
        } else {
            Some(version)
        };

        if self.replace_project_section() {
            return Project { version, ..project };
        }

        let Some(current_project) = current_project else {
            return Project { version, ..project };
        };

        Project {
            name: current_project.name.or(project.name),
            version: current_project.version.or(version),
            description: current_project.description.or(project.description),
            authors: current_project.authors.or(project.authors),
            requires_python: current_project.requires_python.or(project.requires_python),
            readme: current_project.readme.or(project.readme),
            license: current_project.license.or(project.license),
            maintainers: current_project.maintainers.or(project.maintainers),
            keywords: current_project.keywords.or(project.keywords),
            classifiers: current_project.classifiers.or(project.classifiers),
            dependencies: current_project.dependencies.or(project.dependencies),
            optional_dependencies: current_project
                .optional_dependencies
                .or(project.optional_dependencies),
            urls: current_project.urls.or(project.urls),
            scripts: current_project.scripts.or(project.scripts),
            gui_scripts: current_project.gui_scripts.or(project.gui_scripts),
            entry_points: current_project.entry_points.or(project.entry_points),
            dynamic: current_project.dynamic.or(project.dynamic),
            remaining_fields: current_project.remaining_fields,
        }
    }

    /// Name of the current package manager.
    fn get_package_manager_name(&self) -> String;

    /// Get the options chosen by the user to perform the migration, such as the project path,
    /// whether locking should be performed at the end of the migration, ...
    fn get_converter_options(&self) -> &ConverterOptions;

    /// Path to the project to migrate.
    fn get_project_path(&self) -> PathBuf {
        self.get_converter_options().clone().project_path
    }

    /// Whether to perform the migration in dry-run mode, meaning that the changes are printed out
    /// instead of made for real.
    fn is_dry_run(&self) -> bool {
        self.get_converter_options().dry_run
    }

    /// Whether to skip dependencies locking at the end of the migration.
    fn skip_lock(&self) -> bool {
        self.get_converter_options().skip_lock
    }

    /// Whether to replace existing `[project]` section of `pyproject.toml`, or to keep existing
    /// fields.
    fn replace_project_section(&self) -> bool {
        self.get_converter_options().replace_project_section
    }

    /// Whether to keep current build backend.
    fn keep_current_build_backend(&self) -> bool {
        self.get_converter_options().keep_current_build_backend
    }

    /// Whether to keep current package manager data at the end of the migration.
    fn keep_old_metadata(&self) -> bool {
        self.get_converter_options().keep_old_metadata
    }

    /// Whether to perform the migration even if there are errors.
    fn ignore_errors(&self) -> bool {
        self.get_converter_options().ignore_errors
    }

    /// Whether to keep versions locked in the current package manager (if it supports lock files)
    /// when locking dependencies with uv.
    fn respect_locked_versions(&self) -> bool {
        !self.get_converter_options().ignore_locked_versions
    }

    /// Dependency groups strategy to use when writing development dependencies in dependency
    /// groups.
    fn get_dependency_groups_strategy(&self) -> Option<DependencyGroupsStrategy> {
        self.get_converter_options().dependency_groups_strategy
    }

    /// List of files tied to the current package manager to delete at the end of the migration.
    fn get_migrated_files_to_delete(&self) -> Vec<String>;

    /// Delete files tied to the current package manager at the end of the migration, unless user
    /// has chosen to keep the current package manager data.
    fn delete_migrated_files(&self) -> std::io::Result<()> {
        if self.keep_old_metadata() {
            return Ok(());
        }

        for file in self.get_migrated_files_to_delete() {
            let path = self.get_project_path().join(file);

            if path.exists() {
                remove_file(path)?;
            }
        }

        Ok(())
    }

    /// Lock dependencies with uv, unless user has explicitly opted out of locking dependencies.
    fn lock_dependencies(&self) -> Result<(), ()> {
        let lock_type = if self.respect_locked_versions() {
            LockType::LockWithConstraints
        } else {
            LockType::LockWithoutConstraints
        };

        if self.skip_lock() {
            return Ok(());
        }

        uv::lock_dependencies(self.get_project_path().as_ref(), &lock_type)
    }

    /// Get dependencies constraints to set in `constraint-dependencies` under `[tool.uv]` section,
    /// to keep dependencies locked to the same versions as they are with the current package
    /// manager.
    fn get_constraint_dependencies(&self) -> Option<Vec<String>>;

    /// Remove `constraint-dependencies` from `[tool.uv]` in `pyproject.toml`, unless user has
    /// opted out of keeping versions locked in the current package manager.
    ///
    /// Also lock dependencies, to remove `constraints` from `[manifest]` in lock file, unless user
    /// has opted out of locking dependencies.
    fn remove_constraint_dependencies(&self, updated_pyproject_toml: String) {
        if !self.respect_locked_versions() {
            return;
        }

        let mut pyproject_updater = PyprojectUpdater {
            pyproject: &mut updated_pyproject_toml.parse::<DocumentMut>().unwrap(),
        };
        if let Some(updated_pyproject) = pyproject_updater.remove_constraint_dependencies() {
            let mut pyproject_file =
                File::create(self.get_project_path().join("pyproject.toml")).unwrap();
            pyproject_file
                .write_all(updated_pyproject.to_string().as_bytes())
                .unwrap();

            // Lock dependencies a second time, to remove constraints from lock file.
            if !self.skip_lock()
                && uv::lock_dependencies(
                    self.get_project_path().as_ref(),
                    &LockType::ConstraintsRemoval,
                )
                .is_err()
            {
                warn!("An error occurred while locking dependencies after removing constraints.");
            }
        }
    }
}

#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DependencyGroupsStrategy {
    SetDefaultGroupsAll,
    SetDefaultGroups,
    IncludeInDev,
    KeepExisting,
    MergeIntoDev,
}

#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildBackend {
    Hatch,
    Uv,
}

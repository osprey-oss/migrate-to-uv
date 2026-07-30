mod build_backend;
mod dependencies;
mod project;
mod sources;
pub mod version;

use crate::converters::ConverterOptions;
use crate::converters::poetry::build_backend::{BuildBackendObject, get_build_backend};
use crate::converters::poetry::project::get_classifiers;
use crate::converters::pyproject_updater::PyprojectUpdater;
use crate::converters::{Converter, DEFAULT_PROJECT_NAME};
use crate::errors::{
    MIGRATION_ERRORS, MigrationError, add_recoverable_error, add_unrecoverable_error,
};
use crate::schema::pep_621::{License, Project};
use crate::schema::poetry::PoetryLock;
use crate::schema::pyproject::PyProject;
use crate::schema::uv::{SourceContainer, Uv};
use crate::toml::PyprojectPrettyFormatter;
use indexmap::IndexMap;
use owo_colors::OwoColorize;
use std::fs;
use toml_edit::DocumentMut;
use toml_edit::visit_mut::VisitMut;

#[derive(Debug, PartialEq, Eq)]
pub struct Poetry {
    pub converter_options: ConverterOptions,
}

impl Converter for Poetry {
    fn build_uv_pyproject(&self) -> String {
        let pyproject_toml_content =
            fs::read_to_string(self.get_project_path().join("pyproject.toml")).unwrap_or_default();
        let pyproject: PyProject = toml::from_str(pyproject_toml_content.as_str()).unwrap();

        let poetry = pyproject
            .tool
            .unwrap_or_default()
            .poetry
            .unwrap_or_default();

        let build_backend = get_build_backend(
            &self.converter_options,
            pyproject.build_system.as_ref(),
            &poetry,
        );
        let build_system = build_backend::get_new_build_system(
            pyproject.build_system,
            self.converter_options.keep_current_build_backend,
            build_backend.as_ref(),
            self.converter_options.build_backend,
        );

        let mut uv_source_index: IndexMap<String, SourceContainer> = IndexMap::new();
        let (dependency_groups, uv_default_groups) =
            dependencies::get_dependency_groups_and_default_groups(
                &poetry,
                &mut uv_source_index,
                self.get_dependency_groups_strategy(),
            );
        let mut poetry_dependencies = poetry.dependencies;

        let python_specification = poetry_dependencies
            .as_mut()
            .and_then(|dependencies| dependencies.shift_remove("python"));

        let requires_python = python_specification.and_then(|p| match p.to_pep_508() {
            Ok(v) => Some(v),
            Err(e) => {
                add_unrecoverable_error(e.format("python"));
                None
            }
        });

        let optional_dependencies =
            dependencies::get_optional(&mut poetry_dependencies, poetry.extras);

        let mut poetry_plugins = poetry.plugins;
        let scripts_from_plugins = poetry_plugins
            .as_mut()
            .and_then(|plugins| plugins.shift_remove("console_scripts"));
        let gui_scripts = poetry_plugins
            .as_mut()
            .and_then(|plugins| plugins.shift_remove("gui_scripts"));

        let project = Project {
            // "name" is required by uv, and must not be empty.
            name: Some(poetry.name.unwrap_or(DEFAULT_PROJECT_NAME.to_string())),
            description: poetry.description,
            authors: project::get_authors(poetry.authors),
            requires_python: requires_python.clone(),
            readme: project::get_readme(poetry.readme),
            license: poetry.license.map(License::String),
            maintainers: project::get_authors(poetry.maintainers),
            keywords: poetry.keywords,
            classifiers: get_classifiers(
                poetry.classifiers,
                build_system.as_ref(),
                requires_python,
                pyproject.project.as_ref(),
            ),
            dependencies: dependencies::get(poetry_dependencies.as_ref(), &mut uv_source_index),
            optional_dependencies,
            urls: project::get_urls(
                poetry.urls,
                poetry.homepage,
                poetry.repository,
                poetry.documentation,
            ),
            scripts: project::get_scripts(poetry.scripts, scripts_from_plugins),
            gui_scripts,
            entry_points: poetry_plugins,
            ..Default::default()
        };

        let uv = Uv {
            package: poetry.package_mode,
            index: sources::get_indexes(poetry.source),
            sources: if uv_source_index.is_empty() {
                None
            } else {
                Some(uv_source_index)
            },
            default_groups: uv_default_groups,
            constraint_dependencies: self.get_constraint_dependencies(),
            build_backend: if let Some(BuildBackendObject::Uv(ref uv)) = build_backend {
                Some(uv.clone())
            } else {
                None
            },
        };

        let mut updated_pyproject = pyproject_toml_content.parse::<DocumentMut>().unwrap();
        let mut pyproject_updater = PyprojectUpdater {
            pyproject: &mut updated_pyproject,
        };

        pyproject_updater.insert_build_system(build_system.as_ref());
        pyproject_updater.insert_pep_621(&self.build_project(
            pyproject.project,
            project,
            poetry.version.unwrap_or_else(|| "0.0.1".to_string()),
        ));
        pyproject_updater.insert_dependency_groups(dependency_groups.as_ref());
        pyproject_updater.insert_uv(&uv);

        if let Some(BuildBackendObject::Hatch(ref hatch)) = build_backend {
            pyproject_updater.insert_hatch(Some(hatch));
        }

        self.remove_pyproject_poetry_section(&mut updated_pyproject);

        if let Some(build_backend) = build_backend {
            add_recoverable_error(format!(
                "Build backend was migrated to {build_backend}. It is highly recommended to check that files and data included in the source distribution and wheels are the same after the migration."
            ));
        }

        let mut visitor = PyprojectPrettyFormatter::default();
        visitor.visit_document_mut(&mut updated_pyproject);

        updated_pyproject.to_string()
    }

    fn get_package_manager_name(&self) -> String {
        "Poetry".to_string()
    }

    fn get_converter_options(&self) -> &ConverterOptions {
        &self.converter_options
    }

    fn get_migrated_files_to_delete(&self) -> Vec<String> {
        vec!["poetry.lock".to_string(), "poetry.toml".to_string()]
    }

    fn get_constraint_dependencies(&self) -> Option<Vec<String>> {
        let poetry_lock_path = self.get_project_path().join("poetry.lock");

        if self.is_dry_run() || !self.respect_locked_versions() || !poetry_lock_path.exists() {
            return None;
        }

        let poetry_lock_content = fs::read_to_string(poetry_lock_path).unwrap();
        let Ok(poetry_lock) = toml::from_str::<PoetryLock>(poetry_lock_content.as_str()) else {
            MIGRATION_ERRORS.lock().unwrap().push(
                MigrationError::new(
                    format!(
                        "\"{}\" could not be parsed, so dependencies were not kept to their previous locked versions.",
                        "poetry.lock".bold(),
                    ),
                    true,
                )
            );
            return None;
        };

        let constraint_dependencies: Vec<String> = poetry_lock
            .package
            .unwrap_or_default()
            .iter()
            .map(|p| format!("{}=={}", p.name, p.version))
            .collect();

        if constraint_dependencies.is_empty() {
            None
        } else {
            Some(constraint_dependencies)
        }
    }
}

impl Poetry {
    /// Remove `[tool.poetry]` section from `pyproject.toml`, unless user has explicitly asked for
    /// the old metadata to be kept.
    /// If the current build backend should be kept, instead of removing `[tool.poetry]` section, we
    /// only remove the keys that are not related to the build backend.
    fn remove_pyproject_poetry_section(&self, pyproject: &mut DocumentMut) {
        if self.keep_old_metadata() {
            return;
        }

        if let Some(tool) = pyproject.get_mut("tool")
            && let Some(tool_table) = tool.as_table_mut()
        {
            if self.keep_current_build_backend() {
                if let Some(poetry) = tool_table.get_mut("poetry")
                    && let Some(poetry_table) = poetry.as_table_mut()
                {
                    let keys_to_keep = ["packages", "include", "exclude"];
                    let mut found_keys_to_keep = false;

                    for (key, _) in &poetry_table.clone() {
                        if keys_to_keep.contains(&key) {
                            found_keys_to_keep = true;
                        } else {
                            poetry_table.remove(key);
                        }
                    }

                    // If none of the keys to keep was found, remove the entire section.
                    if !found_keys_to_keep {
                        tool_table.remove("poetry");
                    }
                }
            } else {
                tool_table.remove("poetry");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::DependencyGroupsStrategy;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    macro_rules! apply_filters {
        {} => {
            let mut settings = insta::Settings::clone_current();
            settings.add_filter(r"uv_build>=[\d\.]+,<[\d\.]+", "uv_build>=[LOWER_BOUND],<[UPPER_BOUND]");
            let _bound = settings.bind_to_scope();
        }
    }

    #[test]
    fn test_readme_empty_array() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r"
        [tool.poetry]
        readme = []
        ";

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "project"
        version = "0.0.1"
        "#);
    }

    #[test]
    fn test_perform_migration_license_text() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
        [project]
        license = { text = "MIT" }

        [tool.poetry.dependencies]
        python = "^3.12"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "project"
        version = "0.0.1"
        requires-python = ">=3.12,<4"
        license = { text = "MIT" }
        "#);
    }

    #[test]
    fn test_perform_migration_license_file() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
        [project]
        license = { file = "LICENSE" }

        [tool.poetry.dependencies]
        python = "^3.12"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "project"
        version = "0.0.1"
        requires-python = ">=3.12,<4"
        license = { file = "LICENSE" }
        "#);
    }

    #[test]
    fn test_dynamic_version() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
        [project]
        dynamic = ["version"]

        [tool.poetry]
        name = "foo"
        version = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        dynamic = ["version"]
        "#);
    }

    #[test]
    fn test_dynamic_version_no_poetry_version() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
        [project]
        dynamic = ["version"]

        [tool.poetry]
        name = "foo"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        dynamic = ["version"]
        "#);
    }

    #[test]
    fn test_dynamic_version_replace_project_section() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
        [project]
        dynamic = ["version"]

        [tool.poetry]
        name = "foo"
        version = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                replace_project_section: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "1.2.3"
        "#);
    }

    #[test]
    fn test_dynamic_version_no_poetry_version_replace_project_section() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
        [project]
        dynamic = ["version"]

        [tool.poetry]
        name = "foo"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                replace_project_section: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        "#);
    }

    #[test]
    fn test_classifiers_no_python() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        apply_filters!();
        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [build-system]
        requires = ["uv_build>=[LOWER_BOUND],<[UPPER_BOUND]"]
        build-backend = "uv_build"

        [project]
        name = "project"
        version = "0.0.1"
        classifiers = [
            "Programming Language :: Python :: 2",
            "Programming Language :: Python :: 2.7",
            "Programming Language :: Python :: 3",
            "Programming Language :: Python :: 3.4",
            "Programming Language :: Python :: 3.5",
            "Programming Language :: Python :: 3.6",
            "Programming Language :: Python :: 3.7",
            "Programming Language :: Python :: 3.8",
            "Programming Language :: Python :: 3.9",
            "Programming Language :: Python :: 3.10",
            "Programming Language :: Python :: 3.11",
            "Programming Language :: Python :: 3.12",
            "Programming Language :: Python :: 3.13",
            "Programming Language :: Python :: 3.14",
        ]
        "#);
    }

    #[test]
    fn test_classifiers_python_restricted() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"

[tool.poetry.dependencies]
python = "^3.10"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        apply_filters!();
        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [build-system]
        requires = ["uv_build>=[LOWER_BOUND],<[UPPER_BOUND]"]
        build-backend = "uv_build"

        [project]
        name = "project"
        version = "0.0.1"
        requires-python = ">=3.10,<4"
        classifiers = [
            "Programming Language :: Python :: 3",
            "Programming Language :: Python :: 3.10",
            "Programming Language :: Python :: 3.11",
            "Programming Language :: Python :: 3.12",
            "Programming Language :: Python :: 3.13",
            "Programming Language :: Python :: 3.14",
        ]
        "#);
    }

    #[test]
    fn test_classifiers_python_restricted_2() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"

[tool.poetry.dependencies]
python = ">=3.2,<3.13"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        apply_filters!();
        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [build-system]
        requires = ["uv_build>=[LOWER_BOUND],<[UPPER_BOUND]"]
        build-backend = "uv_build"

        [project]
        name = "project"
        version = "0.0.1"
        requires-python = ">=3.2,<3.13"
        classifiers = [
            "Programming Language :: Python :: 3",
            "Programming Language :: Python :: 3.4",
            "Programming Language :: Python :: 3.5",
            "Programming Language :: Python :: 3.6",
            "Programming Language :: Python :: 3.7",
            "Programming Language :: Python :: 3.8",
            "Programming Language :: Python :: 3.9",
            "Programming Language :: Python :: 3.10",
            "Programming Language :: Python :: 3.11",
            "Programming Language :: Python :: 3.12",
        ]
        "#);
    }

    #[test]
    fn test_classifiers_python_restricted_3() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"

[tool.poetry.dependencies]
python = ">=2.6"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        apply_filters!();
        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [build-system]
        requires = ["uv_build>=[LOWER_BOUND],<[UPPER_BOUND]"]
        build-backend = "uv_build"

        [project]
        name = "project"
        version = "0.0.1"
        requires-python = ">=2.6"
        classifiers = [
            "Programming Language :: Python :: 2",
            "Programming Language :: Python :: 2.7",
            "Programming Language :: Python :: 3",
            "Programming Language :: Python :: 3.4",
            "Programming Language :: Python :: 3.5",
            "Programming Language :: Python :: 3.6",
            "Programming Language :: Python :: 3.7",
            "Programming Language :: Python :: 3.8",
            "Programming Language :: Python :: 3.9",
            "Programming Language :: Python :: 3.10",
            "Programming Language :: Python :: 3.11",
            "Programming Language :: Python :: 3.12",
            "Programming Language :: Python :: 3.13",
            "Programming Language :: Python :: 3.14",
        ]
        "#);
    }

    #[test]
    fn test_classifiers_merge_existing() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"

[tool.poetry]
classifiers = [
    "Intended Audience :: Developers",
    "Programming Language :: Python :: 3.13",
    "Programming Language :: Python :: 3.14",
    "Topic :: Software Development :: Libraries",
]

[tool.poetry.dependencies]
python = ">=3.10"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        apply_filters!();
        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [build-system]
        requires = ["uv_build>=[LOWER_BOUND],<[UPPER_BOUND]"]
        build-backend = "uv_build"

        [project]
        name = "project"
        version = "0.0.1"
        requires-python = ">=3.10"
        classifiers = [
            "Intended Audience :: Developers",
            "Programming Language :: Python :: 3.13",
            "Programming Language :: Python :: 3.14",
            "Topic :: Software Development :: Libraries",
            "Programming Language :: Python :: 3",
            "Programming Language :: Python :: 3.10",
            "Programming Language :: Python :: 3.11",
            "Programming Language :: Python :: 3.12",
        ]
        "#);
    }

    #[test]
    fn test_classifiers_no_build_system() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
name = ""
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = ""
        version = "0.0.1"
        "#);
    }

    #[test]
    fn test_classifiers_non_poetry_build_system() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["foo"]
build-backend = "bar"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "project"
        version = "0.0.1"


        [build-system]
        requires = ["foo"]
        build-backend = "bar"
        "#);
    }

    #[test]
    fn test_classifiers_pep_621_no_python() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"

[project]
name = ""
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        apply_filters!();
        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [build-system]
        requires = ["uv_build>=[LOWER_BOUND],<[UPPER_BOUND]"]
        build-backend = "uv_build"

        [project]
        name = ""
        version = "0.0.1"
        classifiers = [
            "Programming Language :: Python :: 2",
            "Programming Language :: Python :: 2.7",
            "Programming Language :: Python :: 3",
            "Programming Language :: Python :: 3.4",
            "Programming Language :: Python :: 3.5",
            "Programming Language :: Python :: 3.6",
            "Programming Language :: Python :: 3.7",
            "Programming Language :: Python :: 3.8",
            "Programming Language :: Python :: 3.9",
            "Programming Language :: Python :: 3.10",
            "Programming Language :: Python :: 3.11",
            "Programming Language :: Python :: 3.12",
            "Programming Language :: Python :: 3.13",
            "Programming Language :: Python :: 3.14",
        ]
        "#);
    }

    #[test]
    fn test_classifiers_pep_621_python_restricted() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"

[project]
requires-python = ">=3.10"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        apply_filters!();
        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [build-system]
        requires = ["uv_build>=[LOWER_BOUND],<[UPPER_BOUND]"]
        build-backend = "uv_build"

        [project]
        name = "project"
        version = "0.0.1"
        requires-python = ">=3.10"
        classifiers = [
            "Programming Language :: Python :: 3",
            "Programming Language :: Python :: 3.10",
            "Programming Language :: Python :: 3.11",
            "Programming Language :: Python :: 3.12",
            "Programming Language :: Python :: 3.13",
            "Programming Language :: Python :: 3.14",
        ]
        "#);
    }

    #[test]
    fn test_classifiers_pep_621_with_classifiers() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"

[project]
classifiers = [
    "Intended Audience :: Developers",
    "Programming Language :: Python :: 3.13",
    "Programming Language :: Python :: 3.14",
    "Topic :: Software Development :: Libraries",
]
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        apply_filters!();
        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [build-system]
        requires = ["uv_build>=[LOWER_BOUND],<[UPPER_BOUND]"]
        build-backend = "uv_build"

        [project]
        name = "project"
        version = "0.0.1"
        classifiers = [
            "Intended Audience :: Developers",
            "Programming Language :: Python :: 3.13",
            "Programming Language :: Python :: 3.14",
            "Topic :: Software Development :: Libraries",
        ]
        "#);
    }

    #[test]
    fn test_keep_current_build_backend() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"

[project]
name = "foo"

[tool.poetry]
# A comment that shoud be preserved
packages = [{ include = "foo" }]
# A comment that shoud be preserved
include = ["foo.txt"]
# A comment that shoud be preserved
exclude = ["bar.txt"]
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                keep_current_build_backend: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"


        [build-system]
        requires = ["poetry-core>=1.0.0"]
        build-backend = "poetry.core.masonry.api"

        [tool.poetry]
        # A comment that shoud be preserved
        packages = [{ include = "foo" }]
        # A comment that shoud be preserved
        include = ["foo.txt"]
        # A comment that shoud be preserved
        exclude = ["bar.txt"]
        "#);
    }

    #[test]
    fn test_keep_current_build_backend_no_keys_to_keep() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[build-system]
requires = ["poetry-core>=1.0.0"]
build-backend = "poetry.core.masonry.api"

[project]
name = "foo"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                keep_current_build_backend: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"


        [build-system]
        requires = ["poetry-core>=1.0.0"]
        build-backend = "poetry.core.masonry.api"
        "#);
    }

    #[test]
    fn test_keep_current_build_backend_no_build_backend() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[project]
name = "foo"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                keep_current_build_backend: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        "#);
    }

    #[test]
    fn test_normalize_dependencies_for_extras() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
name = "foo"
version = "0.0.1"
description = ""
authors = []

[tool.poetry.dependencies]
foo-bar = { version = "1.2.3", optional = true }
bAr_FoO = { version = "3.2.1", optional = true }
foo = "1.2.3"

[tool.poetry.extras]
extra1 = ["foo_BAR"]
extra2 = ["bar-foo"]
extra3 = ["foo-bar", "bar-foo"]
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                keep_current_build_backend: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        description = ""
        authors = []
        dependencies = ["foo==1.2.3"]

        [project.optional-dependencies]
        extra1 = ["foo-bar==1.2.3"]
        extra2 = ["bAr_FoO==3.2.1"]
        extra3 = [
            "foo-bar==1.2.3",
            "bAr_FoO==3.2.1",
        ]
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_none() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = ["bar==1.2.3"]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        default-groups = "all"
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_none_with_include_groups() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.dev]
include-groups = ["typing", "profiling"]

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = [
            "bar==1.2.3",
            { include-group = "typing" },
            { include-group = "profiling" },
        ]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        default-groups = "all"
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_none_with_optional() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling]
optional = true

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = ["bar==1.2.3"]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        default-groups = [
            "dev",
            "typing",
        ]
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_set_default_groups_all() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling]
optional = true

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::SetDefaultGroupsAll),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = ["bar==1.2.3"]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        default-groups = "all"
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_set_default_groups_all_with_include_groups() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.dev]
include-groups = ["typing", "profiling"]

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::SetDefaultGroupsAll),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = [
            "bar==1.2.3",
            { include-group = "typing" },
            { include-group = "profiling" },
        ]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        default-groups = "all"
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_set_default_groups() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling]
optional = true

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::SetDefaultGroups),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = ["bar==1.2.3"]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        default-groups = [
            "dev",
            "typing",
        ]
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_set_default_groups_with_include_groups() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.dev]
include-groups = ["typing", "profiling"]

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::SetDefaultGroups),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = [
            "bar==1.2.3",
            { include-group = "typing" },
            { include-group = "profiling" },
        ]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        default-groups = [
            "dev",
            "typing",
            "profiling",
        ]
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_include_in_dev() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling]
optional = true

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::IncludeInDev),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = [
            "bar==1.2.3",
            { include-group = "typing" },
        ]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_include_in_dev_with_include_groups() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.dev]
include-groups = ["typing"]

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::IncludeInDev),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = [
            "bar==1.2.3",
            { include-group = "typing" },
            { include-group = "profiling" },
        ]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_keep_existing() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling]
optional = true

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::KeepExisting),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = ["bar==1.2.3"]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_keep_existing_with_include_groups() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.dev]
include-groups = ["typing"]

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"

[tool.poetry.group.profiling]
optional = true
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::KeepExisting),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = [
            "bar==1.2.3",
            { include-group = "typing" },
        ]
        typing = ["foobar==1.2.3"]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_merge_into_dev() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling]
optional = true

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::MergeIntoDev),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = [
            "bar==1.2.3",
            "foobar==1.2.3",
        ]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        "#);
    }

    #[test]
    fn test_dependency_groups_strategy_merge_into_dev_with_include_groups() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let pyproject_content = r#"
[tool.poetry]
package-mode = false
name = "foo"

[tool.poetry.dependencies]
python = "^3.11"
foo = "1.2.3"

[tool.poetry.group.dev.dependencies]
bar = "1.2.3"

[tool.poetry.group.dev]
include-groups = ["typing"]

[tool.poetry.group.typing.dependencies]
foobar = "1.2.3"

[tool.poetry.group.profiling.dependencies]
barfoo = "1.2.3"

[tool.poetry.group.profiling]
optional = true
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let poetry = Poetry {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                dependency_groups_strategy: Some(DependencyGroupsStrategy::MergeIntoDev),
                ..Default::default()
            },
        };

        insta::assert_snapshot!(poetry.build_uv_pyproject(), @r#"
        [project]
        name = "foo"
        version = "0.0.1"
        requires-python = ">=3.11,<4"
        dependencies = ["foo==1.2.3"]

        [dependency-groups]
        dev = [
            "bar==1.2.3",
            "foobar==1.2.3",
        ]
        profiling = ["barfoo==1.2.3"]

        [tool.uv]
        package = false
        "#);
    }
}

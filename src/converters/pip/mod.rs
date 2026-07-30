mod dependencies;

use crate::converters::ConverterOptions;
use crate::converters::pyproject_updater::PyprojectUpdater;
use crate::converters::{Converter, DEFAULT_PROJECT_NAME};
use crate::schema::pep_621::Project;
use crate::schema::pyproject::{DependencyGroupSpecification, PyProject};
use crate::schema::uv::Uv;
use crate::toml::PyprojectPrettyFormatter;
use indexmap::IndexMap;
use std::default::Default;
use std::fs;
use toml_edit::DocumentMut;
use toml_edit::visit_mut::VisitMut;

#[derive(Debug, PartialEq, Eq)]
pub struct Pip {
    pub converter_options: ConverterOptions,
    pub requirements_files: Vec<String>,
    pub dev_requirements_files: Vec<String>,
    pub is_pip_tools: bool,
}

impl Converter for Pip {
    fn build_uv_pyproject(&self) -> String {
        let pyproject_toml_content =
            fs::read_to_string(self.get_project_path().join("pyproject.toml")).unwrap_or_default();
        let pyproject: PyProject = toml::from_str(pyproject_toml_content.as_str()).unwrap();

        let dev_dependencies = dependencies::get(
            &self.get_project_path(),
            self.dev_requirements_files.clone(),
        );

        let dependency_groups = dev_dependencies.map(|dependencies| {
            IndexMap::from([(
                "dev".to_string(),
                dependencies
                    .iter()
                    .map(|dep| DependencyGroupSpecification::String(dep.clone()))
                    .collect(),
            )])
        });

        let project = Project {
            // "name" is required by uv, and must not be empty.
            name: Some(DEFAULT_PROJECT_NAME.to_string()),
            dependencies: dependencies::get(
                &self.get_project_path(),
                self.requirements_files.clone(),
            ),
            ..Default::default()
        };

        let uv = Uv {
            package: Some(false),
            constraint_dependencies: self.get_constraint_dependencies(),
            ..Default::default()
        };

        let pyproject_toml_content =
            fs::read_to_string(self.get_project_path().join("pyproject.toml")).unwrap_or_default();
        let mut updated_pyproject = pyproject_toml_content.parse::<DocumentMut>().unwrap();
        let mut pyproject_updater = PyprojectUpdater {
            pyproject: &mut updated_pyproject,
        };

        pyproject_updater.insert_pep_621(&self.build_project(
            pyproject.project,
            project,
            "0.0.1".to_string(),
        ));
        pyproject_updater.insert_dependency_groups(dependency_groups.as_ref());
        pyproject_updater.insert_uv(&uv);

        let mut visitor = PyprojectPrettyFormatter::default();
        visitor.visit_document_mut(&mut updated_pyproject);

        updated_pyproject.to_string()
    }

    fn get_package_manager_name(&self) -> String {
        if self.is_pip_tools {
            return "pip-tools".to_string();
        }
        "pip".to_string()
    }

    fn get_converter_options(&self) -> &ConverterOptions {
        &self.converter_options
    }

    fn respect_locked_versions(&self) -> bool {
        // There are no locked dependencies for pip, so locked versions are only respected for
        // pip-tools.
        self.is_pip_tools && !self.get_converter_options().ignore_locked_versions
    }

    fn get_migrated_files_to_delete(&self) -> Vec<String> {
        let mut files_to_delete: Vec<String> = Vec::new();

        for requirements_file in self
            .requirements_files
            .iter()
            .chain(&self.dev_requirements_files)
        {
            files_to_delete.push(requirements_file.clone());

            // For pip-tools, also delete `.txt` files generated from `.in` files.
            if self.is_pip_tools {
                files_to_delete.push(requirements_file.replace(".in", ".txt"));
            }
        }

        files_to_delete
    }

    fn get_constraint_dependencies(&self) -> Option<Vec<String>> {
        if !self.is_pip_tools || self.is_dry_run() || !self.respect_locked_versions() {
            return None;
        }

        if let Some(dependencies) = dependencies::get(
            self.get_project_path().as_path(),
            self.requirements_files
                .clone()
                .into_iter()
                .chain(self.dev_requirements_files.clone())
                .map(|f| f.replace(".in", ".txt"))
                .collect(),
        ) {
            if dependencies.is_empty() {
                return None;
            }
            return Some(dependencies);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::pip::Pip;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_dynamic_version() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let requirements_content = "foo==1.2.3";

        let mut requirements_file = File::create(project_path.join("requirements.txt")).unwrap();
        requirements_file
            .write_all(requirements_content.as_bytes())
            .unwrap();

        let pyproject_content = r#"
        [project]
        dependencies = ["foo==1.2.3"]
        dynamic = ["version"]
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let pipenv = Pip {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                ..Default::default()
            },
            requirements_files: vec!["requirements.txt".to_string()],
            dev_requirements_files: Vec::new(),
            is_pip_tools: false,
        };

        insta::assert_snapshot!(pipenv.build_uv_pyproject(), @r#"
        [project]
        name = "project"
        dependencies = ["foo==1.2.3"]
        dynamic = ["version"]

        [tool.uv]
        package = false
        "#);
    }

    #[test]
    fn test_dynamic_version_replace_project_section() {
        let tmp_dir = tempdir().unwrap();
        let project_path = tmp_dir.path();

        let requirements_content = "foo==1.2.3";

        let mut requirements_file = File::create(project_path.join("requirements.txt")).unwrap();
        requirements_file
            .write_all(requirements_content.as_bytes())
            .unwrap();

        let pyproject_content = r#"
        [project]
        dynamic = ["version"]
        "#;

        let mut pyproject_file = File::create(project_path.join("pyproject.toml")).unwrap();
        pyproject_file
            .write_all(pyproject_content.as_bytes())
            .unwrap();

        let pipenv = Pip {
            converter_options: ConverterOptions {
                project_path: PathBuf::from(project_path),
                dry_run: true,
                skip_lock: true,
                ignore_locked_versions: true,
                replace_project_section: true,
                ..Default::default()
            },
            requirements_files: vec!["requirements.txt".to_string()],
            dev_requirements_files: Vec::new(),
            is_pip_tools: false,
        };

        insta::assert_snapshot!(pipenv.build_uv_pyproject(), @r#"
        [project]
        name = "project"
        version = "0.0.1"
        dependencies = ["foo==1.2.3"]

        [tool.uv]
        package = false
        "#);
    }
}

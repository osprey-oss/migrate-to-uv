use crate::common::{apply_filters, cli};
use dircpy::copy_dir;
use insta_cmd::assert_cmd_snapshot;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

mod common;

const FIXTURES_PATH: &str = "tests/fixtures";

#[test]
fn test_revert_changes_no_pyproject() {
    let fixture_path = Path::new(FIXTURES_PATH).join("pipenv/lock_file_conflicts");

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    apply_filters!();
    assert_cmd_snapshot!(cli().arg(project_path), @r#"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    Locking dependencies with constraints from existing lock file(s) using "uv lock"...
    warning: The `requires-python` specifier (`~=3.13`) in `project` uses the tilde specifier (`~=`) without a patch version. This will be interpreted as `>=3.13, <4`. Did you mean `~=3.13.0` to constrain the version as `>=3.13.0, <3.14`? We recommend only using the tilde specifier with a patch version to avoid ambiguity.
    Using [PYTHON_INTERPRETER]
      × No solution found when resolving dependencies:
      ╰─▶ Because there is no version of certifi==2026.1.1 and requests==2.30.0
          depends on certifi==2026.1.1, we can conclude that requests==2.30.0
          cannot be used.
          And because your project depends on requests==2.30.0, we can conclude
          that your project's requirements are unsatisfiable.
    error: Could not lock dependencies, aborting the migration. Consider using "--ignore-locked-versions" if you don't need to keep versions from the lock file, or "--skip-lock" if you don't want to lock dependencies at all.
    "#);

    // Assert that previous package manager files have not been removed.
    assert!(project_path.join("Pipfile").exists());
    assert!(project_path.join("Pipfile.lock").exists());

    // Assert that `pyproject.toml` was correctly removed.
    assert!(!project_path.join("pyproject.toml").exists());
}

#[test]
fn test_revert_changes_existing_pyproject() {
    let fixture_path = Path::new(FIXTURES_PATH).join("pipenv/lock_file_conflicts_with_pyproject");

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    let old_pyproject = fs::read_to_string(project_path.join("pyproject.toml")).unwrap();

    apply_filters!();
    assert_cmd_snapshot!(cli().arg(project_path), @r#"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    Locking dependencies with constraints from existing lock file(s) using "uv lock"...
    warning: The `requires-python` specifier (`~=3.13`) in `project` uses the tilde specifier (`~=`) without a patch version. This will be interpreted as `>=3.13, <4`. Did you mean `~=3.13.0` to constrain the version as `>=3.13.0, <3.14`? We recommend only using the tilde specifier with a patch version to avoid ambiguity.
    Using [PYTHON_INTERPRETER]
      × No solution found when resolving dependencies:
      ╰─▶ Because there is no version of certifi==2026.1.1 and requests==2.30.0
          depends on certifi==2026.1.1, we can conclude that requests==2.30.0
          cannot be used.
          And because your project depends on requests==2.30.0, we can conclude
          that your project's requirements are unsatisfiable.
    error: Could not lock dependencies, aborting the migration. Consider using "--ignore-locked-versions" if you don't need to keep versions from the lock file, or "--skip-lock" if you don't want to lock dependencies at all.
    "#);

    // Assert that previous package manager files have not been removed.
    assert!(project_path.join("Pipfile").exists());
    assert!(project_path.join("Pipfile.lock").exists());

    // Assert that `pyproject.toml` has the same content as before the migration.
    assert_eq!(
        fs::read_to_string(project_path.join("pyproject.toml")).unwrap(),
        old_pyproject
    );
}

use crate::common::{LockedPackage, UvLock, apply_filters, cli};
use dircpy::copy_dir;
use insta_cmd::assert_cmd_snapshot;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

mod common;

const FIXTURES_PATH: &str = "tests/fixtures/pip_tools";

#[test]
fn test_complete_workflow() {
    let fixture_path = Path::new(FIXTURES_PATH).join("with_lock_file");
    let requirements_files = [
        "requirements.in",
        "requirements.txt",
        "requirements-dev.in",
        "requirements-dev.txt",
        "requirements-typing.in",
        "requirements-typing.txt",
    ];

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    apply_filters!();
    assert_cmd_snapshot!(cli()
        .arg(project_path)
        .arg("--dev-requirements-file")
        .arg("requirements-dev.in")
        .arg("--dev-requirements-file")
        .arg("requirements-typing.in"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Locking dependencies with constraints from existing lock file(s) using "uv lock"...
    Using [PYTHON_INTERPRETER]
    warning: No `requires-python` value found in the workspace. Defaulting to `[PYTHON_VERSION]`.
    Resolved [PACKAGES] packages in [TIME]
    Locking dependencies again using "uv lock" to remove constraints...
    Using [PYTHON_INTERPRETER]
    warning: No `requires-python` value found in the workspace. Defaulting to `[PYTHON_VERSION]`.
    Resolved [PACKAGES] packages in [TIME]
    Successfully migrated project from pip-tools to uv!
    "#);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = [
        "factory-boy>=3.2.1",
        "mypy>=1.13.0",
    ]

    [tool.uv]
    package = false
    "#);

    let uv_lock = toml::from_str::<UvLock>(
        fs::read_to_string(project_path.join("uv.lock"))
            .unwrap()
            .as_str(),
    )
    .unwrap();

    // Assert that locked versions in `uv.lock` match what was in requirements files.
    let uv_lock_packages = uv_lock.package.unwrap();
    let expected_locked_packages = Vec::from([
        LockedPackage {
            name: "project".to_string(),
            version: "0.0.1".to_string(),
        },
        LockedPackage {
            name: "arrow".to_string(),
            version: "1.2.3".to_string(),
        },
        LockedPackage {
            name: "factory-boy".to_string(),
            version: "3.2.1".to_string(),
        },
        LockedPackage {
            name: "faker".to_string(),
            version: "33.1.0".to_string(),
        },
        LockedPackage {
            name: "mypy".to_string(),
            version: "1.13.0".to_string(),
        },
        LockedPackage {
            name: "mypy-extensions".to_string(),
            version: "1.0.0".to_string(),
        },
        LockedPackage {
            name: "python-dateutil".to_string(),
            version: "2.7.0".to_string(),
        },
        LockedPackage {
            name: "six".to_string(),
            version: "1.15.0".to_string(),
        },
        LockedPackage {
            name: "typing-extensions".to_string(),
            version: "4.6.0".to_string(),
        },
    ]);
    for package in expected_locked_packages {
        assert!(uv_lock_packages.contains(&package));
    }

    // Assert that previous package manager files are correctly removed.
    for file in requirements_files {
        assert!(!project_path.join(file).exists());
    }
}

#[test]
fn test_ignore_locked_versions() {
    let fixture_path = Path::new(FIXTURES_PATH).join("with_lock_file");
    let requirements_files = [
        "requirements.in",
        "requirements.txt",
        "requirements-dev.in",
        "requirements-dev.txt",
        "requirements-typing.in",
        "requirements-typing.txt",
    ];

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    apply_filters!();
    assert_cmd_snapshot!(cli()
        .arg(project_path)
        .arg("--dev-requirements-file")
        .arg("requirements-dev.in")
        .arg("--dev-requirements-file")
        .arg("requirements-typing.in")
        .arg("--ignore-locked-versions"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Locking dependencies using "uv lock"...
    Using [PYTHON_INTERPRETER]
    warning: No `requires-python` value found in the workspace. Defaulting to `[PYTHON_VERSION]`.
    Resolved [PACKAGES] packages in [TIME]
    Successfully migrated project from pip-tools to uv!
    "#);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = [
        "factory-boy>=3.2.1",
        "mypy>=1.13.0",
    ]

    [tool.uv]
    package = false
    "#);

    let uv_lock = toml::from_str::<UvLock>(
        fs::read_to_string(project_path.join("uv.lock"))
            .unwrap()
            .as_str(),
    )
    .unwrap();

    let mut arrow: Option<LockedPackage> = None;
    let mut typing_extensions: Option<LockedPackage> = None;
    for package in uv_lock.package.unwrap() {
        if package.name == "arrow" {
            arrow = Some(package);
        } else if package.name == "typing-extensions" {
            typing_extensions = Some(package);
        }
    }

    // Assert that locked versions are different that what was in `poetry.lock`.
    assert_ne!(arrow.unwrap().version, "1.2.3");
    assert_ne!(typing_extensions.unwrap().version, "4.6.0");

    // Assert that previous package manager files are correctly removed.
    for file in requirements_files {
        assert!(!project_path.join(file).exists());
    }
}

#[test]
fn test_keep_current_data() {
    let fixture_path = Path::new(FIXTURES_PATH).join("with_lock_file");
    let requirements_files = [
        "requirements.in",
        "requirements.txt",
        "requirements-dev.in",
        "requirements-dev.txt",
        "requirements-typing.in",
        "requirements-typing.txt",
    ];

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    apply_filters!();
    assert_cmd_snapshot!(cli()
        .arg(project_path)
        .arg("--dev-requirements-file")
        .arg("requirements-dev.in")
        .arg("--dev-requirements-file")
        .arg("requirements-typing.in")
        .arg("--keep-current-data"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Locking dependencies with constraints from existing lock file(s) using "uv lock"...
    Using [PYTHON_INTERPRETER]
    warning: No `requires-python` value found in the workspace. Defaulting to `[PYTHON_VERSION]`.
    Resolved [PACKAGES] packages in [TIME]
    Locking dependencies again using "uv lock" to remove constraints...
    Using [PYTHON_INTERPRETER]
    warning: No `requires-python` value found in the workspace. Defaulting to `[PYTHON_VERSION]`.
    Resolved [PACKAGES] packages in [TIME]
    Successfully migrated project from pip-tools to uv!
    "#);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = [
        "factory-boy>=3.2.1",
        "mypy>=1.13.0",
    ]

    [tool.uv]
    package = false
    "#);

    // Assert that previous package manager files have not been removed.
    for file in requirements_files {
        assert!(project_path.join(file).exists());
    }
}

#[test]
fn test_skip_lock() {
    let fixture_path = Path::new(FIXTURES_PATH).join("with_lock_file");
    let requirements_files = [
        "requirements.in",
        "requirements.txt",
        "requirements-dev.in",
        "requirements-dev.txt",
        "requirements-typing.in",
        "requirements-typing.txt",
    ];

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    assert_cmd_snapshot!(cli()
        .arg(project_path)
        .arg("--dev-requirements-file")
        .arg("requirements-dev.in")
        .arg("--dev-requirements-file")
        .arg("requirements-typing.in")
        .arg("--skip-lock"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Successfully migrated project from pip-tools to uv!
    "###);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = [
        "factory-boy>=3.2.1",
        "mypy>=1.13.0",
    ]

    [tool.uv]
    package = false
    "#);

    // Assert that previous package manager files are correctly removed.
    for file in requirements_files {
        assert!(!project_path.join(file).exists());
    }

    // Assert that `uv.lock` file was not generated.
    assert!(!project_path.join("uv.lock").exists());
}

#[test]
fn test_skip_lock_full() {
    let fixture_path = Path::new(FIXTURES_PATH).join("full");
    let requirements_files = [
        "requirements.in",
        "requirements.txt",
        "requirements-dev.in",
        "requirements-dev.txt",
        "requirements-typing.in",
        "requirements-typing.txt",
    ];

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    assert_cmd_snapshot!(cli()
        .arg(project_path)
        .arg("--dev-requirements-file")
        .arg("requirements-dev.in")
        .arg("--dev-requirements-file")
        .arg("requirements-typing.in")
        .arg("--skip-lock"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Successfully migrated project from pip-tools to uv!
    "###);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = [
        "arrow",
        "httpx[cli,zstd]==0.28.1",
        "uvicorn @ git+https://github.com/encode/uvicorn",
    ]

    [dependency-groups]
    dev = [
        "pytest>=8.3.4",
        "ruff==0.8.4",
        "mypy==1.14.1",
        "types-jsonschema==4.23.0.20241208",
    ]

    [tool.uv]
    package = false
    "#);

    // Assert that previous package manager files are correctly removed.
    for file in requirements_files {
        assert!(!project_path.join(file).exists());
    }

    // Assert that `uv.lock` file was not generated.
    assert!(!project_path.join("uv.lock").exists());
}

#[test]
fn test_dry_run() {
    let project_path = Path::new(FIXTURES_PATH).join("with_lock_file");
    let requirements_files = [
        "requirements.in",
        "requirements.txt",
        "requirements-dev.in",
        "requirements-dev.txt",
        "requirements-typing.in",
        "requirements-typing.txt",
    ];

    assert_cmd_snapshot!(cli()
        .arg(&project_path)
        .arg("--dev-requirements-file")
        .arg("requirements-dev.in")
        .arg("--dev-requirements-file")
        .arg("requirements-typing.in")
        .arg("--dry-run"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Migrated pyproject.toml:
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = [
        "factory-boy>=3.2.1",
        "mypy>=1.13.0",
    ]

    [tool.uv]
    package = false
    "#);

    // Assert that previous package manager files have not been removed.
    for file in requirements_files {
        assert!(project_path.join(file).exists());
    }

    // Assert that `pyproject.toml` was not created.
    assert!(!project_path.join("pyproject.toml").exists());

    // Assert that `uv.lock` file was not generated.
    assert!(!project_path.join("uv.lock").exists());
}

#[test]
fn test_preserves_existing_project() {
    let project_path = Path::new(FIXTURES_PATH).join("existing_project");

    assert_cmd_snapshot!(cli().arg(&project_path).arg("--dry-run"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Migrated pyproject.toml:
    [project]
    name = "foobar"
    version = "1.0.0"
    requires-python = ">=3.13"
    dependencies = ["arrow>=1.2.3"]

    [tool.uv]
    package = false
    "###);
}

#[test]
fn test_replaces_existing_project() {
    let project_path = Path::new(FIXTURES_PATH).join("existing_project");

    assert_cmd_snapshot!(cli()
        .arg(&project_path)
        .arg("--dry-run")
        .arg("--replace-project-section"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Migrated pyproject.toml:
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [tool.uv]
    package = false
    "#);
}

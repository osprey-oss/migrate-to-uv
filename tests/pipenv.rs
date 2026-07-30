use crate::common::{LockedPackage, UvLock, apply_filters, cli};
use dircpy::copy_dir;
use insta_cmd::assert_cmd_snapshot;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

mod common;

const FIXTURES_PATH: &str = "tests/fixtures/pipenv";

#[test]
fn test_complete_workflow() {
    let fixture_path = Path::new(FIXTURES_PATH).join("with_lock_file");

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    apply_filters!();
    assert_cmd_snapshot!(cli().arg(project_path), @r#"
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
    Successfully migrated project from Pipenv to uv!
    "#);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = ["mypy>=1.13.0"]
    test = ["factory-boy>=3.2.1"]

    [tool.uv]
    package = false
    default-groups = "all"

    [[tool.uv.index]]
    name = "pypi"
    url = "https://pypi.org/simple"
    "#);

    let uv_lock = toml::from_str::<UvLock>(
        fs::read_to_string(project_path.join("uv.lock"))
            .unwrap()
            .as_str(),
    )
    .unwrap();

    // Assert that locked versions in `uv.lock` match what was in `poetry.lock`.
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
    assert!(!project_path.join("Pipfile").exists());
    assert!(!project_path.join("Pipfile.lock").exists());
}

#[test]
fn test_ignore_locked_versions() {
    let fixture_path = Path::new(FIXTURES_PATH).join("with_lock_file");

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    apply_filters!();
    assert_cmd_snapshot!(cli().arg(project_path).arg("--ignore-locked-versions"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Locking dependencies using "uv lock"...
    Using [PYTHON_INTERPRETER]
    warning: No `requires-python` value found in the workspace. Defaulting to `[PYTHON_VERSION]`.
    Resolved [PACKAGES] packages in [TIME]
    Successfully migrated project from Pipenv to uv!
    "#);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = ["mypy>=1.13.0"]
    test = ["factory-boy>=3.2.1"]

    [tool.uv]
    package = false
    default-groups = "all"

    [[tool.uv.index]]
    name = "pypi"
    url = "https://pypi.org/simple"
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

    // Assert that locked versions are different that what was in `Pipfile.lock`.
    assert_ne!(arrow.unwrap().version, "1.2.3");
    assert_ne!(typing_extensions.unwrap().version, "4.6.0");

    // Assert that previous package manager files are correctly removed.
    assert!(!project_path.join("Pipfile").exists());
    assert!(!project_path.join("Pipfile.lock").exists());
}

#[test]
fn test_keep_current_data() {
    let fixture_path = Path::new(FIXTURES_PATH).join("with_lock_file");

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    apply_filters!();
    assert_cmd_snapshot!(cli().arg(project_path).arg("--keep-current-data"), @r#"
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
    Successfully migrated project from Pipenv to uv!
    "#);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = ["mypy>=1.13.0"]
    test = ["factory-boy>=3.2.1"]

    [tool.uv]
    package = false
    default-groups = "all"

    [[tool.uv.index]]
    name = "pypi"
    url = "https://pypi.org/simple"
    "#);

    // Assert that previous package manager files have not been removed.
    assert!(project_path.join("Pipfile").exists());
    assert!(project_path.join("Pipfile.lock").exists());
}

#[test]
fn test_skip_lock() {
    let fixture_path = Path::new(FIXTURES_PATH).join("with_lock_file");

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    assert_cmd_snapshot!(cli().arg(project_path).arg("--skip-lock"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Successfully migrated project from Pipenv to uv!
    "###);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = ["mypy>=1.13.0"]
    test = ["factory-boy>=3.2.1"]

    [tool.uv]
    package = false
    default-groups = "all"

    [[tool.uv.index]]
    name = "pypi"
    url = "https://pypi.org/simple"
    "#);

    // Assert that previous package manager files are correctly removed.
    assert!(!project_path.join("Pipfile").exists());
    assert!(!project_path.join("Pipfile.lock").exists());

    // Assert that `uv.lock` file was not generated.
    assert!(!project_path.join("uv.lock").exists());
}

#[test]
fn test_skip_lock_full() {
    let fixture_path = Path::new(FIXTURES_PATH).join("full");

    let tmp_dir = tempdir().unwrap();
    let project_path = tmp_dir.path();

    copy_dir(fixture_path, project_path).unwrap();

    assert_cmd_snapshot!(cli().arg(project_path).arg("--skip-lock"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Successfully migrated project from Pipenv to uv!
    "###);

    insta::assert_snapshot!(fs::read_to_string(project_path.join("pyproject.toml")).unwrap(), @r#"
    [project]
    name = "project"
    version = "0.0.1"
    requires-python = "~=3.13"
    dependencies = [
        "dep==1.2.3",
        "dep-2==1.2.3",
        "dep-3>=1.2.3",
        "dep-4~=1.2.3",
        "dep-5~=1.2",
        "dep-star",
        "with-version-only==1.2.3",
        "with-version-only-2==1.2.3",
        "with-version-only-star",
        "with-extras[foo, bar]==1.2.3",
        "with-source==1.2.3",
        "local-package",
        "local-package-2",
        "local-package-editable",
        "git",
        "git-ref",
        "markers==1.2.3 ; sys_platform == 'win32'",
        "markers-2==1.2.3 ; os_name == 'nt' and sys_platform != 'darwin' and platform_machine == 'x86_64' and platform_python_implementation == 'CPython' and platform_release == '1.2.3' and platform_system == 'Windows' and platform_version == '1.2.3' and python_version > '3.8' and python_full_version > '3.8.0' and implementation_name != 'pypy' and implementation_version > '3.8' and sys_platform == 'win32'",
    ]

    # This comment should be preserved.
    [tool.ruff]
    fix = true

    [dependency-groups]
    dev = [
        "dev-package==1.2.3",
        "dev-package-local",
        "dev-package-source",
    ]
    packages-category = [
        "category-package==1.2.3",
        "category-package-2==1.2.3",
    ]
    packages-category-2 = [
        "category-2-package==1.2.3",
        "category-2-package-2 ; sys_platform == 'win32'",
    ]

    [tool.ruff.format]
    preview = true

    [tool.uv]
    package = false
    default-groups = "all"

    [[tool.uv.index]]
    name = "pypi"
    url = "https://pypi.org/simple"

    [[tool.uv.index]]
    name = "other-index"
    url = "https://example.com/simple"
    explicit = true

    [tool.uv.sources]
    dev-package-local = { path = "package" }
    dev-package-source = { index = "other-index" }
    category-package-2 = { index = "other-index" }
    category-2-package = { index = "other-index" }
    category-2-package-2 = { git = "https://example.com/foo/bar.git", rev = "v1.2.3" }
    with-source = { index = "other-index" }
    local-package = { path = "package/" }
    local-package-2 = { path = "another-package/", editable = false }
    local-package-editable = { path = "package/dist/package-0.1.0.tar.gz", editable = true }
    git = { git = "https://example.com/foo/bar.git" }
    git-ref = { git = "https://example.com/foo/bar.git", rev = "v1.2.3" }
    "#);

    // Assert that `uv.lock` file was not generated.
    assert!(!project_path.join("uv.lock").exists());
}

#[test]
fn test_dry_run() {
    let project_path = Path::new(FIXTURES_PATH).join("with_lock_file");

    assert_cmd_snapshot!(cli().arg(&project_path).arg("--dry-run"), @r#"
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
    dev = ["mypy>=1.13.0"]
    test = ["factory-boy>=3.2.1"]

    [tool.uv]
    package = false
    default-groups = "all"

    [[tool.uv.index]]
    name = "pypi"
    url = "https://pypi.org/simple"
    "#);

    // Assert that previous package manager files have not been removed.
    assert!(project_path.join("Pipfile").exists());
    assert!(project_path.join("Pipfile.lock").exists());

    // Assert that `pyproject.toml` was not created.
    assert!(!project_path.join("pyproject.toml").exists());

    // Assert that `uv.lock` file was not generated.
    assert!(!project_path.join("uv.lock").exists());
}

#[test]
fn test_dry_run_minimal() {
    let project_path = Path::new(FIXTURES_PATH).join("minimal");

    assert_cmd_snapshot!(cli().arg(&project_path).arg("--dry-run"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Migrated pyproject.toml:
    [project]
    name = "project"
    version = "0.0.1"

    [tool.uv]
    package = false
    "#);

    // Assert that previous package manager files have not been removed.
    assert!(project_path.join("Pipfile").exists());

    // Assert that `pyproject.toml` was not created.
    assert!(!project_path.join("pyproject.toml").exists());

    // Assert that `uv.lock` file was not generated.
    assert!(!project_path.join("uv.lock").exists());
}

#[test]
fn test_preserves_existing_project() {
    let project_path = Path::new(FIXTURES_PATH).join("existing_project");

    assert_cmd_snapshot!(cli().arg(&project_path).arg("--dry-run"), @r#"
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

    [dependency-groups]
    dev = ["mypy>=1.13.0"]
    test = ["factory-boy>=3.2.1"]

    [tool.uv]
    package = false
    default-groups = "all"

    [[tool.uv.index]]
    name = "pypi"
    url = "https://pypi.org/simple"
    "#);
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
    requires-python = "~=3.13"
    dependencies = ["arrow>=1.2.3"]

    [dependency-groups]
    dev = ["mypy>=1.13.0"]
    test = ["factory-boy>=3.2.1"]

    [tool.uv]
    package = false
    default-groups = "all"

    [[tool.uv.index]]
    name = "pypi"
    url = "https://pypi.org/simple"
    "#);
}

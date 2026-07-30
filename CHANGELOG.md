# Changelog

## Unreleased

### Features

* [poetry] Bump default `uv_build` bounds to `>=0.12.0,<0.13` ([#921](https://github.com/osprey-oss/migrate-to-uv/pull/921))

## 0.12.0 - 2026-04-08

The project source code was moved under [a new GitHub organization](https://github.com/osprey-oss/migrate-to-uv). This release does not come with functional changes, but since the documentation has been moved to a [new URL](https://osprey-oss.github.io/migrate-to-uv/), it ensures that links are updated in the CLI and in PyPI metadata. See https://github.com/osprey-oss/migrate-to-uv/issues/823 for details about the move to an organization.

## 0.11.1 - 2026-03-31

### Bug fixes

* Do not set version when setting version dynamically with `project.dynamic` ([#814](https://github.com/osprey-oss/migrate-to-uv/pull/814))
* [pip] Handle multi-line dependencies ([#751](https://github.com/osprey-oss/migrate-to-uv/pull/751))

## 0.11.0 - 2026-02-20

### Breaking changes

#### Automatically choose dependency groups strategy when not set

When migrating Poetry or Pipenv dependency groups, `migrate-to-uv` used to always explicitly set all non-optional dependency groups to `default-groups` (e.g., `default-groups = ["dev", "typing"]`).

If no `--dependency-groups-strategy` option is set, it now defaults to using `default-groups = "all"`, unless at least one dependency group is optional, in which case groups are explicitly added to `default-groups`.

If you wish to keep the previous behavior, to always explicitly set all dependency groups, you can set `--dependency-groups-strategy set-default-groups`.

### Features

* Add `--ignore-errors` flag to perform the migration even in case of errors ([#657](https://github.com/osprey-oss/migrate-to-uv/pull/657))
* Add `set-default-groups-all` option to `--dependency-groups-strategy` ([#708](https://github.com/osprey-oss/migrate-to-uv/pull/708))
* Automatically choose best dependency groups strategy by default ([#711](https://github.com/osprey-oss/migrate-to-uv/pull/711))
* [poetry] Bump default `uv_build` bounds to `>=0.10.0,<0.11.0` ([#683](https://github.com/osprey-oss/migrate-to-uv/pull/683))
* [poetry] Respect `include-groups` from `[tool.poetry.group.<group>]` ([#722](https://github.com/osprey-oss/migrate-to-uv/pull/722))

### Bug fixes

* [poetry] Normalize dependency names when getting dependencies from extras ([#695](https://github.com/osprey-oss/migrate-to-uv/pull/695))

## 0.10.2 - 2026-01-29

### Bug fixes

* [poetry] Respect `--build-backend hatch` when migrating projects that have neither `packages` nor `include` nor `exclude` ([#667](https://github.com/osprey-oss/migrate-to-uv/pull/667))
* [poetry] Correctly set `module-root` when migrating projects without `packages` nor `include` nor `exclude` to uv build backend ([#668](https://github.com/osprey-oss/migrate-to-uv/pull/668))

## 0.10.1 - 2026-01-26

### Bug fixes

* [poetry] Keep explicit sources when using only one primary/default source with explicit sources ([#662](https://github.com/osprey-oss/migrate-to-uv/pull/662))

## 0.10.0 - 2026-01-16

### Breaking changes

#### Automatic selection of build backend

When migrating Poetry package distribution metadata, `migrate-to-uv` now automatically chooses the build backend to use based on the metadata complexity, prioritising [uv](https://docs.astral.sh/uv/concepts/build-backend/) if it is simple enough, or using [Hatch](https://hatch.pypa.io/latest/config/build/) otherwise.

It is still possible to explicitly choose a specific build backend with `--build-backend hatch` or `--build-backend uv`, but if the latter is chosen and the package distribution metadata cannot be expressed with uv build backend, the migration will fail, suggesting to use `--build-backend hatch` instead.

### Features

* Abort migration early if uv executable is required but not found ([#558](https://github.com/osprey-oss/migrate-to-uv/pull/558))
* Improve dependencies locking wording ([#609](https://github.com/osprey-oss/migrate-to-uv/pull/609))
* Add `--keep-current-build-backend` flag ([#614](https://github.com/osprey-oss/migrate-to-uv/pull/614))
* [poetry] Choose build backend based on distribution complexity ([#597](https://github.com/osprey-oss/migrate-to-uv/pull/597))
* [poetry] Automatically add Python classifiers for packages based on `python` specifier ([#606](https://github.com/osprey-oss/migrate-to-uv/pull/606))
* [poetry] Add lower and upper bounds to `uv_build` ([#617](https://github.com/osprey-oss/migrate-to-uv/pull/617))
* [poetry] Enable namespace for packages without `__init__.py` on uv build backend ([#631](https://github.com/osprey-oss/migrate-to-uv/pull/631))
* [poetry] Use more concise warnings output ([#632](https://github.com/osprey-oss/migrate-to-uv/pull/632))

### Bug fixes

* Abort migration on constraints lock failure and revert changes ([#629](https://github.com/osprey-oss/migrate-to-uv/pull/629))
* [poetry] Fix typo on `--build-backend` error message ([#571](https://github.com/osprey-oss/migrate-to-uv/pull/571))
* [poetry] Use `python_full_version` for 3-components Python markers ([#559](https://github.com/osprey-oss/migrate-to-uv/pull/559))
* [poetry] Handle platform markers delimited by pipe ([#576](https://github.com/osprey-oss/migrate-to-uv/pull/576), [#498](https://github.com/osprey-oss/migrate-to-uv/pull/498))
* [poetry] Avoid empty arrays in uv build backend ([#582](https://github.com/osprey-oss/migrate-to-uv/pull/582))
* [poetry] Consistently use `python_full_version` for Python markers to match uv behavior ([#583](https://github.com/osprey-oss/migrate-to-uv/pull/583))
* [poetry] Fail on wheel-only packages using array for uv build backend ([#595](https://github.com/osprey-oss/migrate-to-uv/pull/595))
* [poetry] Abort migration for files using `from` in `packages` ([#567](https://github.com/osprey-oss/migrate-to-uv/pull/567))
* [poetry] Abort migration on `packages` using `from`/`to` and glob ([#615](https://github.com/osprey-oss/migrate-to-uv/pull/615))
* [poetry] Avoid converting empty `extras` for dependencies ([#624](https://github.com/osprey-oss/migrate-to-uv/pull/624))
* [poetry] Avoid setting empty `module-name` for uv build backend ([#627](https://github.com/osprey-oss/migrate-to-uv/pull/627))
* [poetry] Handle `src`-layout with empty packages for uv build backend ([#628](https://github.com/osprey-oss/migrate-to-uv/pull/628))
* [poetry] Sort sources and only set `default` for single `primary`/`default` source ([#633](https://github.com/osprey-oss/migrate-to-uv/pull/633))

## 0.9.1 - 2025-12-24

### Bug fixes

* [poetry] Fail migration on missing `__init__.py` for uv build backend ([#553](https://github.com/osprey-oss/migrate-to-uv/pull/553))

## 0.9.0 - 2025-12-22

### New features

#### Experimental support for uv build backend

When migrating Poetry package distribution metadata, `migrate-to-uv` uses [Hatch](https://hatch.pypa.io/latest/config/build/) build backend. Experimental support for migrating to uv build backend has been added behind `--build backend-uv` argument.

Note that uv build backend offers less flexibility than Poetry and Hatch, so the migration might be aborted if some options used by Poetry cannot be expressed with uv build backend. If you try `--build-backend uv` and encounter any issue, feel free to report it.

### Features

* [poetry] Add experimental `--build-backend uv` argument to migrate package distribution metadata to uv build backend ([#533](https://github.com/osprey-oss/migrate-to-uv/pull/533))

### Bug fixes

* [poetry] Do not migrate `packages`/`include` with empty `format` array ([#538](https://github.com/osprey-oss/migrate-to-uv/pull/538))
* [poetry] Fail migration on unhandled python marker specification ([#544](https://github.com/osprey-oss/migrate-to-uv/pull/544))

## 0.8.1 - 2025-12-13

### Bug fixes

* [poetry] Fail on unhandled version specifications ([#514](https://github.com/osprey-oss/migrate-to-uv/pull/514))
* [poetry] Also check if `poetry.lock` exists when checking if a project uses Poetry ([#528](https://github.com/osprey-oss/migrate-to-uv/pull/528))

## 0.8.0 - 2025-11-17

### Breaking changes

#### Abort on unrecoverable errors and warn about recoverable ones

Although `migrate-to-uv` tries its best to migrate a project to uv without changing the behavior, some things that are accepted by package managers do not have any equivalent in uv. Previously, `migrate-to-uv` would warn about the issue, but still perform the migration. It now aborts the migration in case it is not able to perform the migration that would result in behavior changes when migrating to uv.

If errors occur and lead to aborting the migration, you are expected to manually update your setup and retry the migration.

Warnings that occurred during the migration (which did not break the behavior) are now also grouped and displayed at the very end of the migration.

### Features

* feat!: abort on unrecoverable errors and warn about recoverable ones ([#480](https://github.com/osprey-oss/migrate-to-uv/pull/480))
* [poetry] Do not set optional groups as default ones ([#299](https://github.com/osprey-oss/migrate-to-uv/pull/299))
* Indicate support for Python 3.14 ([#468](https://github.com/osprey-oss/migrate-to-uv/pull/468))

### Bug fixes

* [poetry] Use inclusion when converting `^x.y` versions ([#466](https://github.com/osprey-oss/migrate-to-uv/pull/466))
* [poetry] Properly convert `include` to Hatch's build backend ([#477](https://github.com/osprey-oss/migrate-to-uv/pull/477))
* [poetry] Do not crash on empty `readme` array ([#481](https://github.com/osprey-oss/migrate-to-uv/pull/481))
* [poetry] Abort migration on dependencies using `||` operator, as there is no PEP 440 equivalent ([#487](https://github.com/osprey-oss/migrate-to-uv/pull/487))
* [poetry] Handle versions that use Poetry style and `,`, like `^1.0,!=1.1.0` ([#489](https://github.com/osprey-oss/migrate-to-uv/pull/489))
* [pip/pip-tools] Suggest how to add dependencies that could not be converted ([#350](https://github.com/osprey-oss/migrate-to-uv/pull/350))
* Preserve comments for sections unrelated to migration ([#471](https://github.com/osprey-oss/migrate-to-uv/pull/471))

## 0.7.3 - 2025-06-07

### Bug fixes

* Use correct `include-group` name to include groups when using `--dependency-groups-strategy include-in-dev` ([#283](https://github.com/osprey-oss/migrate-to-uv/pull/283))
* [poetry] Handle `=` single equality ([#288](https://github.com/osprey-oss/migrate-to-uv/pull/288))
* [pipenv] Handle raw versions ([#292](https://github.com/osprey-oss/migrate-to-uv/pull/292))

## 0.7.2 - 2025-03-25

### Bug fixes

* [pipenv] Handle `*` for version ([#212](https://github.com/osprey-oss/migrate-to-uv/pull/212))

## 0.7.1 - 2025-02-22

### Bug fixes

* Handle map for PEP 621 `license` field ([#156](https://github.com/osprey-oss/migrate-to-uv/pull/156))

## 0.7.0 - 2025-02-15

### Features

* Add `--skip-uv-checks` to skip checking if uv is already used in a project ([#118](https://github.com/osprey-oss/migrate-to-uv/pull/118))

### Bug fixes

* [pip/pip-tools] Warn on unhandled dependency formats ([#103](https://github.com/osprey-oss/migrate-to-uv/pull/103))
* [pip/pip-tools] Ignore inline comments when parsing dependencies ([#105](https://github.com/osprey-oss/migrate-to-uv/pull/105))
* [poetry] Migrate scripts that use `scripts = { callable = "foo:run" }` format instead of crashing ([#138](https://github.com/osprey-oss/migrate-to-uv/pull/138))

## 0.6.0 - 2025-01-20

Existing data in `[project]` section of `pyproject.toml` is now preserved by default when migrating. If you prefer that the section is fully replaced, this can be done by setting `--replace-project-section` flag, like so:

```bash
migrate-to-uv --replace-project-section
```

Poetry projects that use PEP 621 syntax to define project metadata, for which support was added in [Poetry 2.0](https://python-poetry.org/blog/announcing-poetry-2.0.0/), are now supported.

### Features

* Preserve existing data in `[project]` section of `pyproject.toml` when migrating ([#84](https://github.com/osprey-oss/migrate-to-uv/pull/84))
* [poetry] Support migrating projects using PEP 621 ([#85](https://github.com/osprey-oss/migrate-to-uv/pull/85))

## 0.5.0 - 2025-01-18

### Features

* [poetry] Delete `poetry.toml` after migration ([#62](https://github.com/osprey-oss/migrate-to-uv/pull/62))
* [pipenv] Delete `Pipfile.lock` after migration ([#66](https://github.com/osprey-oss/migrate-to-uv/pull/66))
* Exit if uv is detected as a package manager ([#61](https://github.com/osprey-oss/migrate-to-uv/pull/61))

### Bug fixes

* Ensure that lock file exists before parsing ([#67](https://github.com/osprey-oss/migrate-to-uv/pull/67))

### Documentation

* Explain how to set credentials for private indexes ([#60](https://github.com/osprey-oss/migrate-to-uv/pull/60))

## 0.4.0 - 2025-01-17

When generating `uv.lock` with `uv lock` command, `migrate-to-uv` now keeps the same versions dependencies were locked to with the previous package manager (if a lock file was found), both for direct and transitive dependencies. This is supported for Poetry, Pipenv, and pip-tools.

This new behavior can be opted out by setting `--ignore-locked-versions` flag, like so:

```bash
migrate-to-uv --ignore-locked-versions
```

### Features

* Keep locked dependencies versions when generating `uv.lock` ([#56](https://github.com/osprey-oss/migrate-to-uv/pull/56))

## 0.3.0 - 2025-01-12

Dependencies are now locked with `uv lock` at the end of the migration, if `uv` is detected as an executable. This new behavior can be opted out by setting `--skip-lock` flag, like so:

```bash
migrate-to-uv --skip-lock
```

### Features

* Lock dependencies at the end of migration ([#46](https://github.com/osprey-oss/migrate-to-uv/pull/46))

## 0.2.1 - 2025-01-05

### Bug fixes

* [poetry] Avoid crashing when an extra lists a non-existing dependency ([#30](https://github.com/osprey-oss/migrate-to-uv/pull/30))

## 0.2.0 - 2025-01-05

### Features

* Support migrating projects using `pip` and `pip-tools` ([#24](https://github.com/osprey-oss/migrate-to-uv/pull/24))
* [poetry] Migrate data from `packages`, `include` and `exclude` to Hatch build backend ([#16](https://github.com/osprey-oss/migrate-to-uv/pull/16))

## 0.1.2 - 2025-01-02

### Bug fixes

* [pipenv] Correctly update `pyproject.toml` ([#19](https://github.com/osprey-oss/migrate-to-uv/pull/19))
* Do not insert `[tool.uv]` if empty ([#17](https://github.com/osprey-oss/migrate-to-uv/pull/17))

## 0.1.1 - 2024-12-26

### Miscellaneous

* Fix documentation publishing and package metadata ([#3](https://github.com/osprey-oss/migrate-to-uv/pull/3))

## 0.1.0 - 2024-12-26

Initial release, with support for Poetry and Pipenv.

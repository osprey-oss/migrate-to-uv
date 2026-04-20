# migrate-to-uv

[![PyPI](https://img.shields.io/pypi/v/migrate-to-uv.svg)](https://pypi.org/project/migrate-to-uv/)
[![License](https://img.shields.io/pypi/l/migrate-to-uv.svg)](https://pypi.org/project/migrate-to-uv/)
[![Supported Python versions](https://img.shields.io/pypi/pyversions/migrate-to-uv.svg)](https://pypi.org/project/migrate-to-uv/)

`migrate-to-uv` migrates a project to [uv](https://github.com/astral-sh/uv) from another package manager.

## Installation

```bash
# To install and build from Rust source code:
cargo install --git https://github.com/mkniewallner/migrate-to-uv

# If you don't want to build from source, install it as a PyPI package, either with
pipx install migrate-to-uv

# or with
uv tool install migrate-to-uv
```

## Usage

```bash
# With uv
uvx migrate-to-uv

# With pipx
pipx run migrate-to-uv
```

## Supported package managers

The following package managers are supported:

- [Poetry](https://python-poetry.org/) (including projects
  using [PEP 621 in Poetry 2.0+](https://python-poetry.org/blog/announcing-poetry-2.0.0/))
- [Pipenv](https://pipenv.pypa.io/en/stable/)
- [pip-tools](https://pip-tools.readthedocs.io/en/stable/)
- [pip](https://pip.pypa.io/en/stable/)

More package managers (e.g., [setuptools](https://setuptools.pypa.io/en/stable/)) could be implemented in the future.

## Features

`migrate-to-uv` converts most existing metadata from supported package managers when migrating to uv, including:

- [Project metadata](https://packaging.python.org/en/latest/guides/writing-pyproject-toml/#writing-pyproject-toml) (`name`, `version`, `authors`, ...)
- [Dependencies and optional dependencies](https://packaging.python.org/en/latest/guides/writing-pyproject-toml/#dependencies-optional-dependencies)
- [Dependency groups](https://packaging.python.org/en/latest/specifications/dependency-groups/#dependency-groups)
- [Dependency sources](https://docs.astral.sh/uv/concepts/projects/dependencies/#dependency-sources) (index, git, URL, path)
- [Dependency markers](https://packaging.python.org/en/latest/specifications/dependency-specifiers/)
- [Entry points](https://packaging.python.org/en/latest/specifications/pyproject-toml/#entry-points)

Version definitions set for dependencies are also preserved, and converted to their
equivalent [PEP 440](https://peps.python.org/pep-0440/) for package managers that use their own syntax (for instance
Poetry's [caret](https://python-poetry.org/docs/dependency-specification/#caret-requirements) syntax).

At the end of the migration, `migrate-to-uv` also generates `uv.lock` file with `uv lock` command to lock dependencies,
and keeps dependencies (both direct and transitive) to the exact same versions they were locked to with the previous
package manager, if a lock file was found.

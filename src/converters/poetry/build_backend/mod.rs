use crate::converters::{BuildBackend, ConverterOptions};
use crate::errors::{add_recoverable_error, add_unrecoverable_error};
use crate::schema::hatch::Hatch;
use crate::schema::poetry::{Format, Poetry};
use crate::schema::pyproject::BuildSystem;
use crate::schema::utils::SingleOrVec;
use crate::schema::uv::UvBuildBackend;
use crate::uv::get_version;
use owo_colors::OwoColorize;
use pep440_rs::Version;
use std::fmt::Display;
use std::str::FromStr;
use std::string::ToString;

pub mod hatch;
pub mod uv;

/// Minimum version to use for `uv_build`. This corresponds to the version that stabilized the build
/// backend (<https://github.com/astral-sh/uv/releases/tag/0.7.19>).
const MIN_UV_BUILD_VERSION: &str = "0.7.19";
/// Default bounds to use for `uv_build` when no version is found.
const UV_BUILD_DEFAULT_BOUNDS: &str = ">=0.12.0,<0.13";

pub enum BuildBackendObject {
    Uv(UvBuildBackend),
    Hatch(Hatch),
}

impl Display for BuildBackendObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uv(_) => write!(f, "uv"),
            Self::Hatch(_) => write!(f, "Hatch"),
        }
    }
}

/// Get the lower and upper bounds to use for `uv_build`, following what the official documentation
/// recommends (<https://docs.astral.sh/uv/concepts/build-backend/#using-the-uv-build-backend>).
///
/// The lower bound corresponds to the version of uv installed by the user, if found, is stable, and
/// is greater or equal to the minimum version of uv that marked `uv_build` as stable.
///
/// If no version is found, we set lower and upper bounds to static versions as best effort, which
/// is better than not setting any bounds at all.
fn get_uv_build() -> String {
    if let Some(raw_version) = get_version()
        && let Ok(version) = Version::from_str(raw_version.as_str())
        && version.is_stable()
        && version > Version::from_str(MIN_UV_BUILD_VERSION).unwrap()
        && let [x, y, _] = version.release()
    {
        return format!("uv_build>={version},<{x}.{}.0", y + 1);
    }

    format!("uv_build{UV_BUILD_DEFAULT_BOUNDS}")
}

pub fn get_new_build_system(
    current_build_system: Option<BuildSystem>,
    keep_current_build_backend: bool,
    new_build_system: Option<&BuildBackendObject>,
    build_backend: Option<BuildBackend>,
) -> Option<BuildSystem> {
    if keep_current_build_backend {
        return None;
    }

    if current_build_system?.build_backend? == "poetry.core.masonry.api" {
        return match new_build_system {
            Some(BuildBackendObject::Hatch(_)) => Some(BuildSystem {
                requires: vec!["hatchling".to_string()],
                build_backend: Some("hatchling.build".to_string()),
            }),
            None if build_backend == Some(BuildBackend::Hatch) => Some(BuildSystem {
                requires: vec!["hatchling".to_string()],
                build_backend: Some("hatchling.build".to_string()),
            }),
            _ => Some(BuildSystem {
                requires: vec![get_uv_build()],
                build_backend: Some("uv_build".to_string()),
            }),
        };
    }

    None
}

/// Get build backend based on converter options. If `--build-backend` is not set or set to `hatch`,
/// Hatch is selected. If `--build-backend` is set to `uv`, Uv is selected.
pub fn get_build_backend(
    converter_options: &ConverterOptions,
    build_system: Option<&BuildSystem>,
    poetry: &Poetry,
) -> Option<BuildBackendObject> {
    if converter_options.keep_current_build_backend {
        return None;
    }

    match &converter_options.build_backend {
        None => {
            let (uv, errors) = uv::get_build_backend(
                poetry.name.as_ref(),
                &converter_options.project_path,
                poetry.packages.as_ref(),
                poetry.include.as_ref(),
                poetry.exclude.as_ref(),
                build_system,
            );

            if errors.is_empty() {
                uv.map(BuildBackendObject::Uv)
            } else {
                add_recoverable_error(
                    "Migrating build backend to Hatch, as package distribution is too complex to be expressed with uv.".to_string()
                );

                let (hatch, errors) = hatch::get_build_backend(
                    &converter_options.project_path,
                    poetry.packages.as_ref(),
                    poetry.include.as_ref(),
                    poetry.exclude.as_ref(),
                );

                if errors.is_empty() {
                    hatch.map(BuildBackendObject::Hatch)
                } else {
                    for error in errors {
                        add_unrecoverable_error(error.clone());
                    }

                    if converter_options.ignore_errors {
                        hatch.map(BuildBackendObject::Hatch)
                    } else {
                        add_unrecoverable_error(format!(
                            "Package distribution could not be migrated to uv nor Hatch build backend due to the issues above. Consider keeping the current build backend with \"{}\".",
                            "--keep-current-build-backend".bold(),
                        ));

                        None
                    }
                }
            }
        }
        Some(BuildBackend::Uv) => {
            let (uv, errors) = uv::get_build_backend(
                poetry.name.as_ref(),
                &converter_options.project_path,
                poetry.packages.as_ref(),
                poetry.include.as_ref(),
                poetry.exclude.as_ref(),
                build_system,
            );

            if errors.is_empty() {
                uv.map(BuildBackendObject::Uv)
            } else {
                for error in errors {
                    add_unrecoverable_error(error.clone());
                }

                if converter_options.ignore_errors {
                    uv.map(BuildBackendObject::Uv)
                } else {
                    add_unrecoverable_error(format!(
                        "Package distribution could not be migrated to uv build backend due to the issues above. Consider using Hatch build backend with \"{}\".",
                        "--build-backend hatch".bold(),
                    ));

                    None
                }
            }
        }
        Some(BuildBackend::Hatch) => {
            let (hatch, errors) = hatch::get_build_backend(
                &converter_options.project_path,
                poetry.packages.as_ref(),
                poetry.include.as_ref(),
                poetry.exclude.as_ref(),
            );

            if errors.is_empty() {
                hatch.map(BuildBackendObject::Hatch)
            } else {
                for error in errors {
                    add_unrecoverable_error(error.clone());
                }

                if converter_options.ignore_errors {
                    hatch.map(BuildBackendObject::Hatch)
                } else {
                    add_unrecoverable_error(format!(
                        "Package distribution could not be migrated to Hatch build backend due to the issues above. Consider keeping the current build backend with \"{}\".",
                        "--keep-current-build-backend".bold(),
                    ));

                    None
                }
            }
        }
    }
}

/// Get the distributions to include an item from `packages` to.
/// <https://python-poetry.org/docs/pyproject/#packages>
fn get_packages_distribution_format(format: Option<&SingleOrVec<Format>>) -> (bool, bool) {
    match format {
        None => (true, true),
        Some(SingleOrVec::Single(Format::Sdist)) => (true, false),
        Some(SingleOrVec::Single(Format::Wheel)) => (false, true),
        // Note: An empty `format = []` in Poetry means that the files will not be added to
        // any distribution at all.
        Some(SingleOrVec::Vec(vec)) => (vec.contains(&Format::Sdist), vec.contains(&Format::Wheel)),
    }
}

/// Get the distributions to include an item from `include` to.
/// <https://python-poetry.org/docs/pyproject/#exclude-and-include>
fn get_include_distribution_format(format: Option<&SingleOrVec<Format>>) -> (bool, bool) {
    match format {
        // If there is no format specified, files are only added to sdist.
        None | Some(SingleOrVec::Single(Format::Sdist)) => (true, false),
        Some(SingleOrVec::Single(Format::Wheel)) => (false, true),
        // Note: An empty `format = []` in Poetry means that the files will not be added to
        // any distribution at all.
        Some(SingleOrVec::Vec(vec)) => (vec.contains(&Format::Sdist), vec.contains(&Format::Wheel)),
    }
}

#[cfg(test)]
mod tests {
    use crate::converters::poetry::build_backend::get_uv_build;
    use crate::uv;
    use pep440_rs::Version;
    use std::str::FromStr;

    #[test]
    fn test_get_uv_build_version_specifier() {
        let lower_bound = Version::from_str(uv::get_version().unwrap().as_str()).unwrap();
        let upper_bound = if let [x, y, _] = lower_bound.release() {
            format!("{x}.{}.0", y + 1)
        } else {
            panic!()
        };

        assert_eq!(
            get_uv_build(),
            format!("uv_build>={lower_bound},<{upper_bound}")
        );
    }
}

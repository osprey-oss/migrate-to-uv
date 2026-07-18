use owo_colors::OwoColorize;
use pep440_rs::{Operator, Version, VersionSpecifiers};
use std::str::FromStr;

pub enum PoetryPep440 {
    String(String),
    Compatible(Version),
    Inclusive(Version, Version),
}

impl PoetryPep440 {
    /// Convert PEP 440 version specifier to Python marker syntax. `python_full_version` is
    /// consistently used over `python_version` to match what uv does.
    pub fn to_python_marker(&self) -> String {
        let pep_440_python = VersionSpecifiers::from_str(self.to_string().as_str()).unwrap();

        pep_440_python
            .iter()
            .map(|spec| {
                let version = match spec.operator() {
                    // For Equal operator, we follow what uv does when running
                    // `uv add <package> --marker "python_version == '<version>'"`.
                    Operator::Equal => match spec.version().release().len() {
                        1 => format!("{}.0.*", spec.version()),
                        2 => format!("{}.*", spec.version()),
                        _ => spec.version().to_string(),
                    },
                    _ => spec.version().to_string(),
                };

                format!("python_full_version {} '{version}'", spec.operator())
            })
            .collect::<Vec<String>>()
            .join(" and ")
    }

    /// <https://python-poetry.org/docs/dependency-specification/#caret-requirements>
    fn from_caret(s: &str) -> Result<Self, ParseVersionError> {
        if let Ok(version) = Version::from_str(s) {
            return match version.clone().release() {
                [0, 0, z] => Ok(Self::Inclusive(version, Version::new([0, 0, z + 1]))),
                [0, y] | [0, y, _, ..] => Ok(Self::Inclusive(version, Version::new([0, y + 1]))),
                [x, _, ..] | [x] => Ok(Self::Inclusive(version, Version::new([x + 1]))),
                [..] => Ok(Self::String(String::new())),
            };
        }
        Err(ParseVersionError::new(
            ParseVersionErrorKind::Other,
            s.to_string(),
        ))
    }

    /// <https://python-poetry.org/docs/dependency-specification/#tilde-requirements>
    fn from_tilde(s: &str) -> Result<Self, ParseVersionError> {
        if let Ok(version) = Version::from_str(s) {
            return match version.clone().release() {
                [_, _, _, ..] => Ok(Self::Compatible(version)),
                [x, y] => Ok(Self::Inclusive(version, Version::new([x, &(y + 1)]))),
                [x] => Ok(Self::Inclusive(version, Version::new([x + 1]))),
                [..] => Ok(Self::String(String::new())),
            };
        }
        Err(ParseVersionError::new(
            ParseVersionErrorKind::Other,
            s.to_string(),
        ))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseVersionErrorKind {
    OrOperator(String),
    Other,
    PythonMarkerOrOperator(String),
    PythonMarker,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseVersionError {
    pub kind: ParseVersionErrorKind,
    pub version: String,
}

impl ParseVersionError {
    pub const fn new(kind: ParseVersionErrorKind, version: String) -> Self {
        Self { kind, version }
    }

    pub fn format(self, dependency: &str) -> String {
        match self.kind {
            ParseVersionErrorKind::OrOperator(operator) => {
                format!(
                    "\"{}\" dependency with version \"{}\" contains \"{}\", which is specific to Poetry and not supported by PEP 440. See https://osprey-oss.github.io/migrate-to-uv/supported-package-managers/#operator for guidance.",
                    dependency.bold(),
                    self.version.bold(),
                    operator.bold(),
                )
            }
            ParseVersionErrorKind::PythonMarkerOrOperator(operator) => {
                format!(
                    "\"{}\" dependency with python marker \"{}\" contains \"{}\", which is specific to Poetry and not supported by PEP 440. See https://osprey-oss.github.io/migrate-to-uv/supported-package-managers/#operator for guidance.",
                    dependency.bold(),
                    self.version.bold(),
                    operator.bold(),
                )
            }
            ParseVersionErrorKind::PythonMarker => {
                format!(
                    "\"{}\" dependency with python marker \"{}\" could not be transformed to PEP 440 format. Make sure to check https://osprey-oss.github.io/migrate-to-uv/supported-package-managers/#unsupported-version-specifiers.",
                    dependency.bold(),
                    self.version.bold(),
                )
            }
            ParseVersionErrorKind::Other => {
                format!(
                    "\"{}\" dependency with version \"{}\" could not be transformed to PEP 440 format. Make sure to check https://osprey-oss.github.io/migrate-to-uv/supported-package-managers/#unsupported-version-specifiers.",
                    dependency.bold(),
                    self.version.bold(),
                )
            }
        }
    }
}

impl FromStr for PoetryPep440 {
    type Err = ParseVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // While Poetry has its own specification for version specifiers, it also supports most of
        // the version specifiers defined by PEP 440. So if the version is a valid PEP 440
        // definition, we can directly use it without any transformation.
        if VersionSpecifiers::from_str(s).is_ok() {
            return Ok(Self::String(s.to_string()));
        }

        // Poetry supports an `||` operator (or `|`, which is equivalent), as in
        // "^1.0 || ^2.0 || ^3.0", but there is no PEP 440 equivalent, so it cannot be converted.
        for operator in ["||", "|"] {
            if s.contains(operator) {
                return Err(ParseVersionError::new(
                    ParseVersionErrorKind::OrOperator(operator.to_string()),
                    s.to_string(),
                ));
            }
        }

        let mut pep_440_specifier = Vec::new();

        // Even when using Poetry-specific version specifiers, it is still possible to define
        // additional PEP 440 specifiers (e.g., "^1.0,!=1.1.0") or even define multiple Poetry
        // specifiers (e.g., "^1.0,^1.1"), so we need to split over "," and treat each group
        // separately, knowing that each group can either be a Poetry-specific specifier, or a PEP
        // 440 one.
        for specifier in s.trim().split(',') {
            let specifier = specifier.trim();

            // If the subgroup is a valid PEP 440 specifier, we can directly use it without any
            // transformation.
            if VersionSpecifiers::from_str(specifier).is_ok() {
                pep_440_specifier.push(Self::String(specifier.to_string()));
            } else {
                let mut chars = specifier.chars();

                match (chars.next(), chars.as_str()) {
                    (Some('*'), "") => pep_440_specifier.push(Self::String(String::new())),
                    (Some('^'), version) => {
                        let v = Self::from_caret(version.trim())?;
                        pep_440_specifier.push(v);
                    }
                    (Some('~'), version) => {
                        let v = Self::from_tilde(version.trim())?;
                        pep_440_specifier.push(v);
                    }
                    (Some('='), version) => {
                        pep_440_specifier.push(Self::String(format!("=={version}")));
                    }
                    (Some('0'..='9'), _) => pep_440_specifier.push(Self::String(format!("=={s}"))),
                    _ => {
                        return Err(ParseVersionError::new(
                            ParseVersionErrorKind::Other,
                            s.to_string(),
                        ));
                    }
                }
            }
        }

        // Concatenate the different specifiers that were split over "," into the final version.
        let version = pep_440_specifier
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join(",");

        // At this point, we should end up with a PEP 440-valid version. If it's not the case, we
        // should error out.
        match VersionSpecifiers::from_str(version.as_str()) {
            Ok(_) => Ok(Self::String(version)),
            Err(_) => Err(ParseVersionError::new(
                ParseVersionErrorKind::Other,
                s.to_string(),
            )),
        }
    }
}

impl std::fmt::Display for PoetryPep440 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match &self {
            Self::String(s) => s.clone(),
            Self::Compatible(version) => format!("~={version}"),
            Self::Inclusive(lower, upper) => format!(">={lower},<{upper}"),
        };

        write!(f, "{str}")
    }
}

use std::fmt::{Display, Formatter};

/// A semantic version (major.minor.patch).
///
/// Used for both FX3 firmware and FPGA versions queried from the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticVersion {
    /// Major version number.
    pub(crate) major: u16,
    /// Minor version number.
    pub(crate) minor: u16,
    /// Patch version number.
    pub(crate) patch: u16,
}

impl std::str::FromStr for SemanticVersion {
    type Err = crate::Error;

    fn from_str(value: &str) -> crate::Result<Self> {
        let mut fields = value.split('-').next().unwrap_or_default().split('.');
        let mut field = || {
            fields
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .ok_or_else(|| crate::Error::Argument("invalid semantic version".into()))
        };
        let version = Self::new(field()?, field()?, field()?);
        if fields.next().is_some() {
            return Err(crate::Error::Argument("invalid semantic version".into()));
        }
        Ok(version)
    }
}
impl SemanticVersion {
    /// Creates a version from its components.
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Major version.
    pub fn major(&self) -> u16 {
        self.major
    }

    /// Minor version.
    pub fn minor(&self) -> u16 {
        self.minor
    }

    /// Patch version.
    pub fn patch(&self) -> u16 {
        self.patch
    }
}

impl Display for SemanticVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}.{}.{}", self.major, self.minor, self.patch))
    }
}

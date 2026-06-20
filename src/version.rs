use std::fmt::{Display, Formatter};

/// A semantic version (major.minor.patch).
///
/// Used for both FX3 firmware and FPGA versions queried from the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticVersion {
    /// Major version number.
    pub(crate) major: u16,
    /// Minor version number.
    pub(crate) minor: u16,
    /// Patch version number.
    pub(crate) patch: u16,
}
impl SemanticVersion {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn major(&self) -> u16 {
        self.major
    }

    pub fn minor(&self) -> u16 {
        self.minor
    }

    pub fn patch(&self) -> u16 {
        self.patch
    }
}

impl Display for SemanticVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}.{}.{}", self.major, self.minor, self.patch))
    }
}

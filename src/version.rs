use std::fmt::{Display, Formatter};
#[derive(Debug)]
pub struct SemanticVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}
impl Display for SemanticVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}.{}.{}", self.major, self.minor, self.patch))
    }
}

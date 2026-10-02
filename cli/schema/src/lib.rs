#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub const SCHEMA_MAJOR: u32 = 1;
pub const SCHEMA_MINOR: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
}

impl SchemaVersion {
    pub const CURRENT: Self = Self {
        major: SCHEMA_MAJOR,
        minor: SCHEMA_MINOR,
    };

    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    pub fn check_major(self) -> Result<(), u32> {
        check_major(self.major)
    }
}

pub fn check_major(found: u32) -> Result<(), u32> {
    if found != SCHEMA_MAJOR {
        return Err(found);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_matches_frozen_constants() {
        assert_eq!(SchemaVersion::CURRENT.major, SCHEMA_MAJOR);
        assert_eq!(SchemaVersion::CURRENT.minor, SCHEMA_MINOR);
        assert_eq!(SchemaVersion::CURRENT, SchemaVersion::new(1, 1));
    }

    #[test]
    fn matching_major_passes_regardless_of_minor() {
        assert_eq!(check_major(1), Ok(()));
        assert_eq!(SchemaVersion::new(1, 0).check_major(), Ok(()));
        assert_eq!(SchemaVersion::new(1, 7).check_major(), Ok(()));
    }

    #[test]
    fn mismatching_major_fails_with_found() {
        assert_eq!(check_major(0), Err(0));
        assert_eq!(check_major(2), Err(2));
        assert_eq!(SchemaVersion::new(2, 0).check_major(), Err(2));
    }
}

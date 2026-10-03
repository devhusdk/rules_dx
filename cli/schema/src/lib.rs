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
    fn matching_major_passes_regardless_of_minor() {
        assert_eq!(check_major(1), Ok(()));
        assert_eq!(check_major(SCHEMA_MAJOR), Ok(()));
    }

    #[test]
    fn mismatching_major_fails_with_found() {
        assert_eq!(check_major(0), Err(0));
        assert_eq!(check_major(2), Err(2));
        assert_eq!(check_major(SCHEMA_MAJOR + 1), Err(SCHEMA_MAJOR + 1));
    }
}

#[cfg(has_cc_optout_stamp)]
pub fn stamped() -> u32 {
    1
}

#[cfg(not(has_cc_optout_stamp))]
pub fn stamped() -> u32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_present() {
        assert_eq!(stamped(), 1);
    }
}

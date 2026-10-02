#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use prost::Message;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderViolation {
    Duplicate,
    Unsorted,
}

pub fn check_sorted_next<T: Ord>(previous: Option<&T>, current: &T) -> Result<(), OrderViolation> {
    match previous {
        Some(prev) if prev == current => Err(OrderViolation::Duplicate),
        Some(prev) if prev > current => Err(OrderViolation::Unsorted),
        _ => Ok(()),
    }
}

pub fn check_unique_insert<T: Ord, E>(
    seen: &mut BTreeSet<T>,
    item: T,
    duplicate: impl FnOnce(&T) -> E,
) -> Result<(), E> {
    if seen.contains(&item) {
        return Err(duplicate(&item));
    }
    seen.insert(item);
    Ok(())
}

pub fn encode_with_validation<M: Message, E>(
    msg: &M,
    validate: impl FnOnce(&M) -> Result<(), E>,
) -> Result<Vec<u8>, E> {
    validate(msg)?;
    Ok(msg.encode_to_vec())
}

pub fn decode_with_validation<M: Message + Default, E>(
    bytes: &[u8],
    validate: impl Fn(&M) -> Result<(), E>,
    map_decode_error: impl FnOnce(String) -> E,
) -> Result<M, E> {
    let msg = M::decode(bytes).map_err(|error| map_decode_error(error.to_string()))?;
    validate(&msg)?;
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_next_accepts_first_and_ascending() {
        assert!(check_sorted_next::<String>(None, &"a".to_owned()).is_ok());
        assert!(check_sorted_next(Some(&"a".to_owned()), &"b".to_owned()).is_ok());
    }

    #[test]
    fn sorted_next_reports_duplicate_and_unsorted() {
        assert_eq!(
            check_sorted_next(Some(&"a".to_owned()), &"a".to_owned()),
            Err(OrderViolation::Duplicate)
        );
        assert_eq!(
            check_sorted_next(Some(&"b".to_owned()), &"a".to_owned()),
            Err(OrderViolation::Unsorted)
        );
    }

    #[test]
    fn unique_insert_rejects_repeat_with_caller_error() {
        let mut seen = BTreeSet::new();
        assert!(check_unique_insert(&mut seen, "a".to_owned(), |_| "dup").is_ok());
        assert_eq!(
            check_unique_insert(&mut seen, "a".to_owned(), |item| format!("dup:{item}")),
            Err("dup:a".to_owned())
        );
    }
}

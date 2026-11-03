//! A ledger of positive amounts with a running total and an Adler-32 checksum.

use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum LedgerError {
    NotPositive(i32),
    Overflow(i32),
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPositive(amount) => write!(f, "amount {amount} is not positive"),
            Self::Overflow(amount) => write!(f, "amount {amount} overflows the total"),
        }
    }
}

#[derive(Debug, Default)]
pub struct Ledger {
    entries: Vec<i32>,
    total: i32,
}

impl Ledger {
    pub fn add(&mut self, amount: i32) -> Result<i32, LedgerError> {
        if amount <= 0 {
            return Err(LedgerError::NotPositive(amount));
        }
        let total = self
            .total
            .checked_add(amount)
            .ok_or(LedgerError::Overflow(amount))?;
        self.entries.push(amount);
        self.total = total;
        Ok(total)
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn total(&self) -> i32 {
        self.total
    }

    pub fn checksum(&self) -> u32 {
        let mut adler = adler2::Adler32::new();
        for entry in &self.entries {
            adler.write_slice(&entry.to_le_bytes());
        }
        adler.checksum()
    }
}

#[cfg(test)]
mod tests {
    use super::{Ledger, LedgerError};

    #[test]
    fn empty_ledger_has_adler_seed() {
        let ledger = Ledger::default();
        assert_eq!(ledger.count(), 0);
        assert_eq!(ledger.total(), 0);
        assert_eq!(ledger.checksum(), 1);
    }

    #[test]
    fn adds_amounts_and_checksums_entries() {
        let mut ledger = Ledger::default();
        assert_eq!(ledger.add(5), Ok(5));
        assert_eq!(ledger.add(7), Ok(12));
        assert_eq!(ledger.count(), 2);
        assert_eq!(ledger.total(), 12);
        assert_eq!(
            ledger.checksum(),
            adler2::adler32_slice(&[5, 0, 0, 0, 7, 0, 0, 0])
        );
        assert_eq!(ledger.checksum(), 0x004c_000d);
    }

    #[test]
    fn rejects_bad_amounts_without_changes() {
        let mut ledger = Ledger::default();
        assert_eq!(ledger.add(0), Err(LedgerError::NotPositive(0)));
        assert_eq!(ledger.add(-3), Err(LedgerError::NotPositive(-3)));
        assert_eq!(ledger.add(i32::MAX), Ok(i32::MAX));
        assert_eq!(ledger.add(1), Err(LedgerError::Overflow(1)));
        assert_eq!(ledger.count(), 1);
        assert_eq!(ledger.total(), i32::MAX);
        assert_eq!(
            LedgerError::Overflow(1).to_string(),
            "amount 1 overflows the total"
        );
    }
}

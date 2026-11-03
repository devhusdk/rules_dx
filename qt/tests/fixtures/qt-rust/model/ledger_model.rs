//! CXX-Qt bridge from the Rust ledger to a QML element.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use ledger::Ledger;

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(i32, count, READ, NOTIFY)]
        #[qproperty(i32, total, READ, NOTIFY)]
        #[qproperty(i64, checksum, READ, NOTIFY)]
        type LedgerModel = super::LedgerRust;

        #[qinvokable]
        fn add(self: Pin<&mut LedgerModel>, amount: i32) -> bool;

        #[qsignal]
        fn rejected(self: Pin<&mut LedgerModel>, amount: i32);
    }
}

pub struct LedgerRust {
    count: i32,
    total: i32,
    checksum: i64,
    ledger: Ledger,
}

impl Default for LedgerRust {
    fn default() -> Self {
        let ledger = Ledger::default();
        Self {
            count: 0,
            total: 0,
            checksum: i64::from(ledger.checksum()),
            ledger,
        }
    }
}

impl qobject::LedgerModel {
    pub fn add(mut self: Pin<&mut Self>, amount: i32) -> bool {
        let mut rust = self.as_mut().rust_mut();
        if rust.ledger.add(amount).is_err() {
            self.as_mut().rejected(amount);
            return false;
        }
        rust.count = i32::try_from(rust.ledger.count()).unwrap_or(i32::MAX);
        rust.total = rust.ledger.total();
        rust.checksum = i64::from(rust.ledger.checksum());
        self.as_mut().count_changed();
        self.as_mut().total_changed();
        self.as_mut().checksum_changed();
        true
    }
}

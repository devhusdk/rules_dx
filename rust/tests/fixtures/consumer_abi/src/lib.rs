#![expect(
    unsafe_code,
    reason = "the fixture exists to call across the C FFI boundary"
)]

use std::ffi::{c_char, CStr};

pub const API_VERSION: i32 = 1;
pub const ERROR_OK: i32 = 0;
pub const ERROR_ARG: i32 = 1;
pub const ERROR_FULL: i32 = 2;
pub const ERROR_EMPTY: i32 = 3;

pub enum AbiStore {}

// SAFETY: the declarations match the C signatures of the abi library.
unsafe extern "C" {
    fn abi_api_version() -> i32;
    fn abi_build_id() -> *const c_char;
    fn abi_arch_bits() -> i32;
    fn abi_create(api_version: i32, capacity: i32) -> *mut AbiStore;
    fn abi_destroy(store: *mut AbiStore);
    fn abi_push(store: *mut AbiStore, value: i32) -> i32;
    fn abi_pop(store: *mut AbiStore, out_value: *mut i32) -> i32;
    fn abi_len(store: *const AbiStore) -> i32;
}

pub struct Store {
    raw: *mut AbiStore,
}

impl Store {
    pub fn create(capacity: i32) -> Option<Store> {
        // SAFETY: abi_create takes plain integers and a null result is handled.
        let raw = unsafe { abi_create(API_VERSION, capacity) };
        if raw.is_null() {
            None
        } else {
            Some(Store { raw })
        }
    }

    pub fn api_version() -> i32 {
        // SAFETY: abi_api_version has no preconditions.
        unsafe { abi_api_version() }
    }

    pub fn build_id() -> String {
        // SAFETY: abi_build_id returns a static NUL-terminated string.
        unsafe {
            CStr::from_ptr(abi_build_id())
                .to_string_lossy()
                .into_owned()
        }
    }

    pub fn arch_bits() -> i32 {
        // SAFETY: abi_arch_bits has no preconditions.
        unsafe { abi_arch_bits() }
    }

    pub fn push(&mut self, value: i32) -> i32 {
        // SAFETY: self.raw is a live store from abi_create, exclusively borrowed.
        unsafe { abi_push(self.raw, value) }
    }

    pub fn pop(&mut self) -> Result<i32, i32> {
        let mut out = 0;
        // SAFETY: self.raw is a live store and out is a valid i32 out-pointer.
        let rc = unsafe { abi_pop(self.raw, &mut out) };
        if rc == ERROR_OK {
            Ok(out)
        } else {
            Err(rc)
        }
    }

    pub fn len(&self) -> i32 {
        // SAFETY: self.raw is a live store from abi_create.
        unsafe { abi_len(self.raw) }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        // SAFETY: self.raw came from abi_create and is destroyed once, in drop.
        unsafe { abi_destroy(self.raw) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_independent_pin() {
        assert_eq!(Store::api_version(), API_VERSION);
    }

    #[test]
    fn build_id_records_independent_toolchain() {
        let id = Store::build_id();
        assert!(id.contains('|'));
        assert_eq!(
            Store::arch_bits(),
            (std::mem::size_of::<*const ()>() * 8) as i32
        );
    }

    #[test]
    fn owns_create_destroy_roundtrip() {
        let mut store = Store::create(2).expect("independent create");
        assert!(store.is_empty());
        assert_eq!(store.push(11), ERROR_OK);
        assert_eq!(store.push(22), ERROR_OK);
        assert_eq!(store.len(), 2);
        assert_eq!(store.push(33), ERROR_FULL);
        assert_eq!(store.pop(), Ok(22));
        assert_eq!(store.pop(), Ok(11));
        assert_eq!(store.pop(), Err(ERROR_EMPTY));
    }

    #[test]
    fn rejects_bad_capacity() {
        assert!(Store::create(0).is_none());
        assert!(Store::create(-4).is_none());
    }
}

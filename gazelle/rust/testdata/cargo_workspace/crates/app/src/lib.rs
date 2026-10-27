use json::Value;
use serde::Serialize;

pub fn run(_value: Value) {
    apiv::ping();
    let _ = libc::getpid;
}

pub fn named<T: Serialize>(_item: T) {}

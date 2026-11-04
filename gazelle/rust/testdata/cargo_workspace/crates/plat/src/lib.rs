#[cfg(target_arch = "aarch64")]
pub use api::ping;

#[cfg(all(windows, target_arch = "x86_64"))]
pub use app::run;

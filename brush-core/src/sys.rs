//! Platform abstraction facilities

#![allow(unused)]

#[cfg(unix)]
pub(crate) mod unix;
#[cfg(all(unix, not(target_os = "twizzler")))]
pub(crate) use unix as platform;

// Twizzler is unix-family, so `unix` above still compiles and most of it is reused; see
// twizzler.rs for which modules come from `stubs` instead and why.
#[cfg(target_os = "twizzler")]
pub(crate) mod twizzler;
#[cfg(target_os = "twizzler")]
pub(crate) use twizzler as platform;

#[cfg(windows)]
pub(crate) mod windows;
#[cfg(windows)]
pub(crate) use windows as platform;

#[cfg(target_family = "wasm")]
pub(crate) mod wasm;
#[cfg(target_family = "wasm")]
pub(crate) use wasm as platform;

#[cfg(any(not(unix), target_os = "twizzler"))]
pub(crate) mod stubs;

#[cfg(any(unix, windows))]
pub(crate) mod hostname;
// Needs tokio's `process` feature, which twizzler does not enable (see Cargo.toml).
#[cfg(all(any(unix, windows), not(target_os = "twizzler")))]
pub mod tokio_process;

pub mod fs;

pub use platform::async_pipe;
pub use platform::commands;
pub(crate) use platform::env;
pub use platform::fd;
pub use platform::input;
pub(crate) use platform::network;
pub use platform::poll;
pub use platform::process;
pub use platform::resource;
pub use platform::signal;
pub use platform::terminal;
pub(crate) use platform::users;

pub use platform::PlatformError;

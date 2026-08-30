//! Twizzler platform support.
//!
//! Twizzler sets `target_family = "unix"`, so the `unix` arm is correct for most of this and is
//! reused directly. Five modules do not come from `unix`, each because its implementation needs
//! something this OS does not provide. Three are taken from `stubs` wholesale; `process` and
//! `signal` are written here, because for those the stub gives up more than this platform
//! requires.
//!
//! - `process` (local): `tokio::process` reaps by waiting on SIGCHLD, which the mlibc sysdeps
//!   never raise, so this is std::process-backed as the stub is. Unlike the stub it reports a
//!   real `id()`: `sys_spawn` hands the child's compartment descriptor back as its pid.
//! - `signal` (local): keeps nix's real `Signal` type and `kill`, replacing only the
//!   delivery-dependent parts. The stub's `Signal` is an uninhabited enum, which breaks callers
//!   that name a signal by constant.
//! - `async_pipe` (stub): built on `tokio::net::unix::pipe`; no `std::os::unix::net` here.
//! - `users` (stub): `uzers` reads a passwd database this system does not have.
//! - `resource` (stub): needs nix's `resource` feature, whose `getrlimit`/`setrlimit`/`getrusage` and
//!   `rlimit` type our libc port does not define.
//!
//! `terminal` deliberately stays on the unix implementation: it compiles, and the calls it makes
//! exist. They are just inert -- `setpgid`/`setsid` return 0 without doing anything and `getpgid`
//! always reports 1 -- so job control silently does nothing rather than failing. See
//! TWIZZLER-PORT.md.
pub use super::stubs::{async_pipe, resource};

pub mod process;
pub mod signal;
// `users`/`env`/`network` are pub(crate) in the unix and stub modules alike; re-exporting them
// as `pub` is an E0365, so the visibility has to match what sys.rs expects.
pub(crate) use super::stubs::users;
pub use super::unix::{PlatformError, commands, fd, fs, input, poll, terminal};
pub(crate) use super::unix::{env, network};

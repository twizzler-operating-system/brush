//! Twizzler platform support.
//!
//! Twizzler sets `target_family = "unix"`, so the `unix` arm is correct for most of this and is
//! reused directly. Six modules do not come from `unix` as-is, each because its implementation
//! needs something this OS does not provide. Two are taken from `stubs` wholesale; `process`,
//! `signal`, `users` and `fs` are written here -- the first three because the stub gives up more
//! than this platform requires, and `fs` because it is `unix`'s with three functions replaced.
//!
//! - `process` (local): `tokio::process` reaps by waiting on SIGCHLD, which the mlibc sysdeps never
//!   raise, so this is std::process-backed as the stub is. Unlike the stub it reports a real
//!   `id()`: `sys_spawn` hands the child's compartment descriptor back as its pid.
//! - `signal` (local): keeps nix's real `Signal` type and `kill`, replacing only the
//!   delivery-dependent parts. The stub's `Signal` is an uninhabited enum, which breaks callers
//!   that name a signal by constant.
//! - `fs` (local): `unix`'s, with the three functions that decide where programs are looked for
//!   replaced -- `*` expansion in `split_paths`, because `/pkg` gains a directory when a package is
//!   installed, and Twizzler-shaped defaults for the other two, whose `unix` answers name Linux
//!   directories that do not exist here.
//! - `async_pipe` (stub): built on `tokio::net::unix::pipe`; no `std::os::unix::net` here.
//! - `users` (local): `uzers` reads a passwd database this system does not have, but the stub's
//!   answer -- an error from every accessor -- reads as a *failure* to callers that have no failure
//!   to report. See users.rs for what is answered instead, and why.
//! - `resource` (stub): needs nix's `resource` feature, whose `getrlimit`/`setrlimit`/`getrusage`
//!   and `rlimit` type our libc port does not define.
//!
//! `terminal` deliberately stays on the unix implementation: it compiles, and the calls it makes
//! exist. They are just inert -- `setpgid`/`setsid` return 0 without doing anything and `getpgid`
//! always reports 1 -- so job control silently does nothing rather than failing. See
//! TWIZZLER-PORT.md.
pub use super::stubs::{async_pipe, resource};

pub mod fs;
pub mod process;
pub mod signal;
// `users`/`env`/`network` are pub(crate) in the unix and stub modules alike; the visibility has
// to match what sys.rs expects (re-exporting one as `pub` is an E0365).
pub(crate) mod users;
pub use super::unix::{PlatformError, commands, fd, input, poll, terminal};
pub(crate) use super::unix::{env, network};

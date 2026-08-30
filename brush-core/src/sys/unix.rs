// Twizzler takes these from `stubs` (see sys/twizzler.rs). Gated out rather than merely
// unused: compiling them would still require `uzers`, `tokio::net::unix::pipe` and
// `tokio::signal`, none of which exist on this target.
#[cfg(not(target_os = "twizzler"))]
pub mod async_pipe;
pub mod commands;
pub(crate) mod env;
pub mod fd;
pub mod fs;
pub mod input;
pub(crate) mod network;
pub mod poll;
use crate::error;
#[cfg(not(target_os = "twizzler"))]
pub use crate::sys::tokio_process as process;
#[cfg(not(target_os = "twizzler"))]
pub mod resource;
#[cfg(not(target_os = "twizzler"))]
pub mod signal;
pub mod terminal;
#[cfg(not(target_os = "twizzler"))]
pub(crate) mod users;

/// Platform-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    /// A system error occurred.
    #[error("system error: {0}")]
    ErrnoError(#[from] nix::errno::Errno),
}

impl From<nix::errno::Errno> for error::ErrorKind {
    fn from(err: nix::errno::Errno) -> Self {
        PlatformError::ErrnoError(err).into()
    }
}

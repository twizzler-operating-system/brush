//! Process management for Twizzler.
//!
//! Shaped like `stubs::process` -- `std::process` underneath, with `wait` async-shaped but
//! blocking, because `tokio::process` reaps via SIGCHLD and mlibc raises none. std's `wait` does
//! work here: it goes through the child's compartment descriptor, which is what mlibc's
//! `sys_waitpid` accepts.
//!
//! The one thing not inherited from the stub is `id()`. The stub returns `None`, which is right
//! for a platform with no process identity at all; here `sys_spawn` hands the child's descriptor
//! back *as* its pid, so there is a real value to return. Without it brush logs "could not
//! retrieve pid for child process" on every external command and, more than cosmetically, has no
//! id to record for the job -- which is what `kill` and `wait` address a child by.
pub(crate) type ProcessId = i32;

/// Provides access to a child process.
pub struct Child {
    inner: std::process::Child,
}

pub(crate) use std::process::{ExitStatus, Output};

impl Child {
    /// Returns the process ID of the child process, if available.
    pub fn id(&self) -> Option<u32> {
        Some(self.inner.id())
    }

    /// Asynchronously waits for the child process to exit.
    pub async fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.inner.wait()
    }

    /// Asynchronously waits for the child process to exit and collects its output.
    pub async fn wait_with_output(self) -> std::io::Result<Output> {
        self.inner.wait_with_output()
    }
}

pub(crate) fn spawn(
    mut command: std::process::Command,
    kill_on_drop: bool,
) -> std::io::Result<Child> {
    // No kill-on-drop here, as in the stub: it would need a way to signal the child, and
    // compartment-wide delivery is not that.
    let _ = kill_on_drop;
    let child = command.spawn()?;
    Ok(Child { inner: child })
}

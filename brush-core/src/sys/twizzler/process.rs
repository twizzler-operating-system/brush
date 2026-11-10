//! Process management for Twizzler.
//!
//! Shaped like `stubs::process` -- `std::process` underneath rather than `tokio::process`, which
//! reaps via SIGCHLD and mlibc raises none. std's `wait` does work here: it goes through the
//! child's compartment descriptor, which is what mlibc's `sys_waitpid` accepts.
//!
//! Two things are not inherited from the stub.
//!
//! `id()`: the stub returns `None`, which is right for a platform with no process identity at
//! all; here `sys_spawn` hands the child's descriptor back *as* its pid, so there is a real value
//! to return. Without it brush logs "could not retrieve pid for child process" on every external
//! command and, more than cosmetically, has no id to record for the job -- which is what `kill`
//! and `wait` address a child by.
//!
//! And the waits run on the blocking pool rather than inline. The stub calls std's blocking
//! `wait` from the body of an `async fn`, which makes a future that never yields `Pending`: it
//! blocks the worker thread inside `poll`. Two callers in `processes.rs` need it to yield, and
//! both are load-bearing here:
//!
//! - `ChildProcess::poll` asks for the child's status with `now_or_never`, i.e. exactly one poll,
//!   expecting `None` while it runs. Inline, that call blocks until the child exits -- so a status
//!   check on a background job waits for the job.
//! - `ChildProcess::wait` selects the child's completion against `await_ctrl_c` and the signal
//!   listeners. A branch that blocks inside `poll` is a branch the others never get polled after,
//!   so ^C could not be observed while a foreground child ran. It mostly went unnoticed because the
//!   PTY posts SIGINT to the child as well (see signal.rs's `mask_sigttou`), so ^C ends the command
//!   by killing it rather than by the shell noticing.
//!
//! `spawn_blocking` is what makes the wait yield; there is no reactor to register with, since a
//! compartment's exit is not a wait word a client can sleep on -- `CompartmentHandle::wait` is a
//! blocking gate call into the monitor, and the runtime's compartment file returns
//! `Unsupported` from `waitpoint`. The thread parks in the blocking pool, which is what that pool
//! is for, instead of holding a worker.
pub(crate) type ProcessId = i32;

/// Provides access to a child process.
pub struct Child {
    /// `None` only while a `wait` is in flight: the std child is moved onto the blocking pool for
    /// the duration and put back when it returns, since `wait` there takes `&mut self` and the
    /// closure must own what it touches.
    inner: Option<std::process::Child>,
}

pub(crate) use std::process::{ExitStatus, Output};

impl Child {
    /// Returns the process ID of the child process, if available.
    pub fn id(&self) -> Option<u32> {
        self.inner.as_ref().map(std::process::Child::id)
    }

    /// Asynchronously waits for the child process to exit.
    pub async fn wait(&mut self) -> std::io::Result<ExitStatus> {
        let Some(mut child) = self.inner.take() else {
            // Only reachable if a previous wait was cancelled mid-flight, which drops the join
            // handle and with it the child; there is nothing left to wait on.
            return Err(std::io::Error::other(
                "child process is already being awaited",
            ));
        };
        let (child, result) = tokio::task::spawn_blocking(move || {
            let result = child.wait();
            (child, result)
        })
        .await
        .map_err(std::io::Error::other)?;
        self.inner = Some(child);
        result
    }

    /// Asynchronously waits for the child process to exit and collects its output.
    pub async fn wait_with_output(mut self) -> std::io::Result<Output> {
        let Some(child) = self.inner.take() else {
            return Err(std::io::Error::other(
                "child process is already being awaited",
            ));
        };
        tokio::task::spawn_blocking(move || child.wait_with_output())
            .await
            .map_err(std::io::Error::other)?
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
    Ok(Child { inner: Some(child) })
}

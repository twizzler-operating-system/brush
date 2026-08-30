//! Signal support for Twizzler.
//!
//! Deliberately *not* the `stubs` version. That one defines `Signal` as an uninhabited enum,
//! which is right for a platform with no signals at all but wrong here: `nix::sys::signal` is
//! ported and works, and brush-builtins constructs `TrapSignal::Signal(nix::…::SIGKILL)`
//! directly, so `sys::signal::Signal` has to *be* nix's type.
//!
//! Only the pieces that need signal *delivery* are replaced -- Twizzler raises no SIGCHLD or
//! SIGTSTP, so the tokio listeners have nothing to wait on.
use crate::{error, sys, traps};

pub(crate) use nix::sys::signal::Signal;

pub(crate) fn continue_process(pid: sys::process::ProcessId) -> Result<(), error::Error> {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), nix::sys::signal::SIGCONT)
        .map_err(|_errno| error::ErrorKind::FailedToSendSignal)?;
    Ok(())
}

/// Sends a signal to a specific process.
pub fn kill_process(
    pid: sys::process::ProcessId,
    signal: traps::TrapSignal,
) -> Result<(), error::Error> {
    let translated_signal = match signal {
        traps::TrapSignal::Signal(signal) => signal,
        traps::TrapSignal::Debug
        | traps::TrapSignal::Err
        | traps::TrapSignal::Exit
        | traps::TrapSignal::Return => {
            return Err(error::ErrorKind::InvalidSignal(signal.to_string()).into());
        }
    };

    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), translated_signal)
        .map_err(|_errno| error::ErrorKind::FailedToSendSignal)?;

    Ok(())
}

pub(crate) fn lead_new_process_group() -> Result<(), error::Error> {
    // mlibc's setpgid returns 0 without doing anything and there is a single process group, so
    // this reports the success the caller expects and changes nothing. See TWIZZLER-PORT.md.
    Ok(())
}

/// Never resolves: nothing raises these here.
pub(crate) struct FakeSignal {}

impl FakeSignal {
    const fn new() -> Self {
        Self {}
    }

    pub async fn recv(&self) {
        futures::future::pending::<()>().await;
    }
}

pub(crate) fn tstp_signal_listener() -> Result<FakeSignal, error::Error> {
    Ok(FakeSignal::new())
}

pub(crate) fn chld_signal_listener() -> Result<FakeSignal, error::Error> {
    Ok(FakeSignal::new())
}

pub(crate) async fn await_ctrl_c() -> std::io::Result<()> {
    FakeSignal::new().recv().await;
    Ok(())
}

pub(crate) fn mask_sigttou() -> Result<(), error::Error> {
    // SIGTTOU is raised when a background process group writes to the controlling terminal.
    // There are no process groups here, so it cannot arrive and there is nothing to mask.
    Ok(())
}

pub(crate) fn poll_for_stopped_children() -> Result<bool, error::Error> {
    // Requires waitid(WUNTRACED) and a notion of a stopped child; neither exists here.
    Ok(false)
}

//! Signal support for Twizzler.
//!
//! Deliberately *not* the `stubs` version. That one defines `Signal` as an uninhabited enum,
//! which is right for a platform with no signals at all but wrong here: `nix::sys::signal` is
//! ported and works, and brush-builtins constructs `TrapSignal::Signal(nix::…::SIGKILL)`
//! directly, so `sys::signal::Signal` has to *be* nix's type.
//!
//! Only the pieces that need signal *delivery* are replaced -- Twizzler raises no SIGCHLD or
//! SIGTSTP, so the tokio listeners have nothing to wait on.
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    thread::Thread,
};

use tokio::sync::watch;

use crate::{error, sys, traps};

pub(crate) use nix::sys::signal::Signal;

/// Count of SIGINTs delivered to this compartment.
///
/// Bumped by [`sigint_handler`], which runs from a Mailbox upcall on the compartment's main
/// thread and may therefore touch nothing that locks or allocates.
static SIGINT_COUNT: AtomicU64 = AtomicU64::new(0);

/// The listener thread, for [`sigint_handler`] to wake.
///
/// Both halves are safe from a signal context: `OnceLock::get` is an atomic load, and `unpark` on
/// a borrowed handle (no refcount traffic) is an atomic swap plus a futex wake -- twizzler selects
/// the futex parker, `library/std/src/sys/sync/thread_parking/futex.rs`. A `Notify` or a `Waker`
/// would not be, which is why the wake is bounced off a thread rather than handed to the async
/// runtime here.
static SIGINT_LISTENER: OnceLock<Thread> = OnceLock::new();

/// Publishes SIGINT arrivals to [`await_ctrl_c`]. Created with the listener thread.
static SIGINT_TX: OnceLock<Arc<watch::Sender<u64>>> = OnceLock::new();

extern "C" fn sigint_handler(_sig: i32) {
    SIGINT_COUNT.fetch_add(1, Ordering::SeqCst);
    if let Some(listener) = SIGINT_LISTENER.get() {
        listener.unpark();
    }
}

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

/// The thread that turns a signal-context wake into an async one.
///
/// Started on the first [`await_ctrl_c`] -- a shell that never runs a foreground command never
/// pays for it -- and parked for the rest of the process.
///
/// Parked, not polling. This used to be a `spawn_blocking` task per wait that woke every 50ms to
/// re-read the counter, and dropping a `JoinHandle` cannot cancel a blocking task: when the child
/// won `ChildProcess::wait`'s select, the loop it left behind kept running forever. Each
/// foreground command therefore added 20 wakeups a second for the life of the shell, and held one
/// of tokio's blocking-pool threads (capped at 512) until the next ^C released them all at once.
fn sigint_channel() -> &'static Arc<watch::Sender<u64>> {
    SIGINT_TX.get_or_init(|| {
        let tx = Arc::new(watch::channel(SIGINT_COUNT.load(Ordering::SeqCst)).0);
        let listener_tx = Arc::clone(&tx);
        // Best-effort: with no listener `await_ctrl_c` never resolves, which is what it does on
        // the platforms whose ctrl-c support is a stub, and ^C still reaches the child directly.
        let _ = std::thread::Builder::new()
            .name("sigint-listener".to_owned())
            .spawn(move || {
                let _ = SIGINT_LISTENER.set(std::thread::current());
                // From the channel rather than a fresh load, so a signal delivered between the
                // channel's creation and here is published rather than swallowed.
                let mut seen = *listener_tx.borrow();
                loop {
                    let now = SIGINT_COUNT.load(Ordering::SeqCst);
                    if now != seen {
                        seen = now;
                        // `send_replace`, not `send`: `send` refuses when no receiver is
                        // subscribed and then leaves the published value behind `seen`, so the
                        // channel would disagree with the counter for every ^C typed at an idle
                        // prompt. Nothing is lost by publishing to nobody.
                        listener_tx.send_replace(now);
                        continue;
                    }
                    // The handler's unpark before this point leaves a token, so the count is
                    // re-read above rather than waited on: no signal can be lost in the window.
                    std::thread::park();
                }
            });
        tx
    })
}

/// Resolves when a SIGINT is delivered after this call.
pub(crate) async fn await_ctrl_c() -> std::io::Result<()> {
    // `subscribe` marks the current value as seen, so this waits for the *next* signal -- what the
    // old snapshot of `SIGINT_COUNT` did. `changed` is cancel-safe and leaves nothing behind when
    // dropped, which is the property that matters here: `ChildProcess::wait` selects on this and
    // drops it every time the child wins.
    let mut rx = sigint_channel().subscribe();
    rx.changed()
        .await
        .map_err(|_| std::io::Error::other("sigint listener stopped"))
}

pub(crate) fn mask_sigttou() -> Result<(), error::Error> {
    // SIGTTOU itself cannot arrive: it is raised when a *background process group* writes to the
    // controlling terminal, and there are no process groups here. But this is the one hook
    // `TerminalControl::acquire` gives us before the shell takes the terminal, and there is a
    // disposition that must be installed before then, so it goes here.
    //
    // On unix the shell needs no SIGINT handler: `lead_new_process_group` puts it in a different
    // process group from its children, so ^C reaches only the foreground job. Twizzler has no
    // process groups. The PTY's ISIG handling instead posts SIGINT to every compartment
    // registered under the PTY's *controller* (`pty_signal_handler` in the reference runtime,
    // with `PostSignalFlags::CONTROLLER`), and children inherit their parent's controller by
    // default -- so the shell receives every ^C alongside the job it is waiting on.
    //
    // The default action for an unhandled SIGINT is `sys_thread_exit(128 + sig)`, so without a
    // disposition here ^C kills the shell rather than the command, and init restarts it. A
    // counting handler both keeps us alive and gives `await_ctrl_c` something to observe.
    let action = nix::sys::signal::SigAction::new(
        nix::sys::signal::SigHandler::Handler(sigint_handler),
        nix::sys::signal::SaFlags::empty(),
        nix::sys::signal::SigSet::empty(),
    );

    // SAFETY: the handler does nothing but an atomic increment, which is async-signal-safe.
    unsafe { nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGINT, &action) }?;

    Ok(())
}

pub(crate) fn poll_for_stopped_children() -> Result<bool, error::Error> {
    // Requires waitid(WUNTRACED) and a notion of a stopped child; neither exists here.
    Ok(false)
}

//! User and group information for Twizzler.
//!
//! There is no passwd database here, and no uid model at all: what a compartment may do is
//! decided by its security context, not by an identity number. So every answer below is a
//! convention rather than a lookup, and the convention is chosen to be *coherent* -- uid 0,
//! `is_root() == true`, and a default username of `root` -- because the alternative fictions are
//! worse in specific ways:
//!
//! - Returning an error, as the `stubs` module does, is what `unix` code does on a real failure,
//!   and callers treat it as one. `\u` in a prompt is the clearest case: `prompt.rs` propagates the
//!   error with `?`, so a `PS1` mentioning the username fails to render *in full* -- not the one
//!   piece. That is the first thing anyone hits pasting a `.bashrc` from Linux.
//! - Leaving `$EUID` unset (the current behaviour, since `wellknownvars` skips it on an error)
//!   turns the standard `[ "$EUID" -eq 0 ]` privilege check into a syntax error rather than a
//!   decision.
//! - Claiming an unprivileged uid would make ported scripts that gate on root refuse to run, on a
//!   system where nothing would have stopped them.
//!
//! The visible consequence is that `\$` renders as `#`, in the default `PS1` among others. That
//! is the honest reading: there is no lesser-privileged mode to drop to.
use std::path::PathBuf;

use crate::error;

/// What `\u` expands to, and the sole entry in the user list.
///
/// `$USER` first, so that anything that sets it -- a login path, a test harness -- is believed
/// over this file.
fn username() -> String {
    std::env::var("USER").unwrap_or_else(|_| String::from("root"))
}

pub(crate) fn get_user_home_dir(username_arg: &str) -> Option<PathBuf> {
    // `~someone-else` has nowhere to resolve to; only the one user exists.
    (username_arg == username())
        .then(get_current_user_home_dir)
        .flatten()
}

pub(crate) fn get_current_user_home_dir() -> Option<PathBuf> {
    std::env::home_dir()
}

pub(crate) fn get_current_user_default_shell() -> Option<PathBuf> {
    // Where `init` launches it from, and so what `$SHELL` should name. The boot-image copy at
    // `/initrd/brush` is the same program.
    Some(PathBuf::from("/pkg/twizzler/bin/brush"))
}

pub(crate) fn is_root() -> bool {
    true
}

pub(crate) fn get_current_uid() -> Result<u32, error::Error> {
    Ok(0)
}

pub(crate) fn get_current_gid() -> Result<u32, error::Error> {
    Ok(0)
}

pub(crate) fn get_effective_uid() -> Result<u32, error::Error> {
    Ok(0)
}

pub(crate) fn get_effective_gid() -> Result<u32, error::Error> {
    Ok(0)
}

pub(crate) fn get_current_username() -> Result<String, error::Error> {
    Ok(username())
}

pub(crate) fn get_user_group_ids() -> Result<Vec<u32>, error::Error> {
    Ok(vec![0])
}

pub(crate) fn get_all_users() -> Result<Vec<String>, error::Error> {
    Ok(vec![username()])
}

pub(crate) fn get_all_groups() -> Result<Vec<String>, error::Error> {
    // Nothing names a group here, so completing one has no candidates -- distinct from failing.
    Ok(vec![])
}

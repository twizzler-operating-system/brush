//! Twizzler filesystem support.
//!
//! `unix`'s, except for the three functions that decide *where* programs are looked for. The
//! `unix` answers describe a Linux filesystem layout (`/usr/bin`, `/bin`, ...) that does not
//! exist here; on this system programs live in `/initrd` and under `/pkg`.
//!
//! The first is `split_paths`: a PATH entry may contain a `*` component, expanded against
//! the filesystem at each lookup. `/pkg` gains a directory whenever a package is installed, so the
//! concrete list of program directories is not known when `init` sets PATH, and a shell that was
//! already running when the package landed would otherwise never see it.
//!
//! The same expansion exists in the runtime's own PATH search (`exec.rs`'s `find_id`, which is
//! what finds a program spawned by bare name from anywhere other than this shell). Duplicated
//! rather than shared: this is a vendored tree in its own cargo workspace, and the twenty lines
//! are not worth a dependency edge from brush onto the Twizzler runtime crates.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub use super::super::unix::fs::*;

/// Splits a PATH-like value into individual paths, expanding any entry with a `*` component.
///
/// Expansion happens as the iterator reaches each entry, not all at once up front: the callers
/// that stop at the first hit -- command lookup, the common case -- resolve most names in
/// `/initrd` or `/pkg/twizzler/bin`, both of which precede the glob, and should not pay to
/// enumerate `/pkg` to do it. Completion, which does want every directory, simply consumes the
/// whole iterator.
pub fn split_paths<T: AsRef<OsStr> + ?Sized>(s: &T) -> impl Iterator<Item = PathBuf> {
    // Collected first so that nothing borrows `s`: `std::env::split_paths` borrows it, and the
    // returned iterator outlives this call.
    let entries: Vec<PathBuf> = std::env::split_paths(s).collect();
    entries
        .into_iter()
        .flat_map(|entry| expand_star(&entry).unwrap_or_else(|| vec![entry]))
}

/// Expands `<prefix>/*/<suffix>` into one path per entry of `<prefix>`.
///
/// `None` means "not a glob, use the entry as it is" -- both for an entry with no `*` and for one
/// whose prefix cannot be read, since a directory that does not exist is exactly what a PATH entry
/// naming a missing package would be, and the caller already handles that.
///
/// Entries are not filtered down to those that exist: the check costs a stat per package on every
/// lookup, while an expansion that names a missing directory costs one failed probe only on the
/// lookups that get that far.
/// The value of PATH when the environment does not carry one.
///
/// `unix` answers `/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin`, none of which
/// exist here. This mirrors what `init` exports, glob included -- the caller joins it into a PATH
/// string that `split_paths` above splits again, so the `*` survives and is expanded per lookup
/// exactly as an inherited PATH's would be.
pub(crate) fn get_default_executable_search_paths() -> Vec<PathBuf> {
    vec![
        "/initrd".into(),
        "/pkg/twizzler/bin".into(),
        "/pkg/*/bin".into(),
    ]
}

/// Where `command -p` looks: the standard utilities, independent of PATH.
///
/// `unix` asks `confstr(_CS_PATH)` first, and mlibc *answers* -- with `/bin:/usr/bin`, which is
/// the one case worse than failing, because a non-empty answer means the hard-coded fallback
/// never runs and `command -p` searches two directories that do not exist. The standard utilities
/// here are uuhelper's coreutils aliases in `/pkg/twizzler/bin`, plus whatever the boot image
/// carries.
///
/// No `*` entry: `command`'s builtin searches this list directly rather than routing it through
/// `split_paths`, so a glob here would be taken literally.
pub fn get_default_standard_utils_paths() -> Vec<PathBuf> {
    vec!["/initrd".into(), "/pkg/twizzler/bin".into()]
}

fn expand_star(path: &Path) -> Option<Vec<PathBuf>> {
    let mut components = path.components();
    let mut prefix = PathBuf::new();
    loop {
        let component = components.next()?;
        if component.as_os_str() == "*" {
            break;
        }
        prefix.push(component);
    }
    let suffix: PathBuf = components.collect();

    let mut expanded: Vec<PathBuf> = std::fs::read_dir(&prefix)
        .ok()?
        .flatten()
        .map(|entry| prefix.join(entry.file_name()).join(&suffix))
        .collect();
    // Directory order is not stable across boots, and PATH order is what decides which of two
    // same-named programs runs.
    expanded.sort();
    Some(expanded)
}

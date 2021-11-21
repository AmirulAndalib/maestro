//! The `getuid` syscall returns the UID of the process's owner.

use crate::errno::Errno;
use crate::process::Process;
use crate::process::Regs;

/// The implementation of the `getuid` syscall.
pub fn getuid(_: &Regs) -> Result<i32, Errno> {
	let mut mutex = Process::get_current().unwrap();
	let mut guard = mutex.lock(false);
	let proc = guard.get_mut();

	Ok(proc.get_uid() as _)
}

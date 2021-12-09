//! The `getpid` system call returns the PID of the current process.

use crate::errno::Errno;
use crate::process::Process;
use crate::process::Regs;

/// The implementation of the `getpid` syscall.
pub fn getpid(_regs: &Regs) -> Result<i32, Errno> {
	let mutex = Process::get_current().unwrap();
	let mut guard = mutex.lock(false);
	let proc = guard.get_mut();

	Ok(proc.get_pid() as _)
}

//! This module implements the `getpgid` system call, which allows to get the process group ID of a
//! process.

use crate::errno::Errno;
use crate::errno;
use crate::process::Process;
use crate::process::pid::Pid;
use crate::process::regs::Regs;

/// The implementation of the `getpgid` syscall.
pub fn getpgid(regs: &Regs) -> Result<i32, Errno> {
	let pid = regs.ebx as Pid;

	let mutex = Process::get_current().unwrap();
	let guard = mutex.lock();
	let proc = guard.get_mut();

	if pid == 0 {
		Ok(proc.get_pgid() as _)
	} else {
		let mutex = {
			if let Some(proc) = Process::get_by_pid(pid) {
				proc
			} else {
				return Err(errno!(ESRCH));
			}
		};
		let guard = mutex.lock();
		let proc = guard.get_mut();

		Ok(proc.get_pgid() as _)
	}
}

/*
 * Copyright 2024 Luc Lenôtre
 *
 * This file is part of Maestro.
 *
 * Maestro is free software: you can redistribute it and/or modify it under the
 * terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or (at your option) any later
 * version.
 *
 * Maestro is distributed in the hope that it will be useful, but WITHOUT ANY
 * WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
 * A PARTICULAR PURPOSE. See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * Maestro. If not, see <https://www.gnu.org/licenses/>.
 */

//! The tkill system call allows to send a signal to a specific thread.

use crate::errno;
use crate::errno::Errno;
use crate::process::pid::Pid;
use crate::process::signal::Signal;
use crate::process::Process;
use core::ffi::c_int;
use macros::syscall;

#[syscall]
pub fn tkill(tid: Pid, sig: c_int) -> Result<i32, Errno> {
	if sig < 0 {
		return Err(errno!(EINVAL));
	}
	let signal = Signal::try_from(sig as u32)?;

	let proc_mutex = Process::current_assert();
	let mut proc = proc_mutex.lock();

	// Check if the thread to kill is the current
	if proc.tid == tid {
		proc.kill(&signal, false);
	} else {
		// Get the thread
		let thread_mutex = Process::get_by_tid(tid).ok_or(errno!(ESRCH))?;
		let mut thread = thread_mutex.lock();

		// Check permissions
		if !proc.access_profile.can_kill(&thread) {
			return Err(errno!(EPERM));
		}

		thread.kill(&signal, false);
	}

	Ok(0)
}

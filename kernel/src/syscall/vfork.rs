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

//! The `vfork` system call works the same as the `fork` system call, except the
//! parent process is blocked until the child process exits or executes a
//! program. During that time, the child process also shares the same memory
//! space as the parent.

use crate::process::{regs::Regs, scheduler, ForkOptions, Process};
use utils::{
	errno::{EResult, Errno},
	lock::IntMutex,
	ptr::arc::Arc,
};

pub fn vfork(proc: &Arc<IntMutex<Process>>, regs: &Regs) -> EResult<usize> {
	let new_pid = {
		let fork_options = ForkOptions {
			vfork: true,
			..ForkOptions::default()
		};
		let new_mutex = proc.lock().fork(Arc::downgrade(proc), fork_options)?;
		let mut new_proc = new_mutex.lock();
		// Set child's return value to `0`
		let mut regs = regs.clone();
		regs.set_syscall_return(Ok(0));
		new_proc.regs = regs;
		new_proc.get_pid()
	};
	// Let another process run instead of the current. Because the current
	// process must now wait for the child process to terminate or execute a program
	scheduler::end_tick();
	// Set parent's return value to the child's PID
	Ok(new_pid as _)
}

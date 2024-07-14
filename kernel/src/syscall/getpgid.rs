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

//! This module implements the `getpgid` system call, which allows to get the
//! process group ID of a process.

use crate::{
	process::{pid::Pid, Process},
	syscall::Args,
};
use utils::{
	errno,
	errno::{EResult, Errno},
};

pub fn getpgid(Args(pid): Args<Pid>) -> EResult<usize> {
	if pid == 0 {
		let proc = Process::current().lock();
		Ok(proc.pgid as _)
	} else {
		let Some(proc) = Process::get_by_pid(pid) else {
			return Err(errno!(ESRCH));
		};
		let proc = proc.lock();
		Ok(proc.pgid as _)
	}
}

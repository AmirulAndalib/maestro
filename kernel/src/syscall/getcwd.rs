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

//! The `getcwd` system call allows to retrieve the current working directory of
//! the current process.

use crate::{
	process::{mem_space::copy::SyscallSlice, Process},
	syscall::Args,
};
use utils::{
	errno,
	errno::{EResult, Errno},
	format,
};

pub fn getcwd(
	Args((buf, size)): Args<(SyscallSlice<u8>, usize)>,
	proc: &Process,
) -> EResult<usize> {
	// Validation
	if size == 0 {
		return Err(errno!(EINVAL));
	}
	// Check that the buffer is large enough
	if size < proc.cwd.0.len() + 1 {
		return Err(errno!(ERANGE));
	}
	// Write
	// TODO do not allocate. write twice instead
	let cwd = format!("{}\0", proc.cwd.0)?;
	buf.copy_to_user(cwd.as_bytes())?;
	Ok(buf.as_ptr() as _)
}

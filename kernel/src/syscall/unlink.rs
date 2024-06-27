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

//! The `unlink` system call deletes the given link from its filesystem.
//!
//! If no link remain to the file, the function also removes it.

use super::{Args, SyscallString};
use crate::{
	file::{path::Path, vfs, vfs::ResolutionSettings},
	process::Process,
};
use utils::{
	errno,
	errno::{EResult, Errno},
};

pub fn unlink(Args(pathname): Args<SyscallString>) -> EResult<usize> {
	let proc_mutex = Process::current_assert();
	let proc = proc_mutex.lock();

	let mem_space_mutex = proc.get_mem_space().unwrap();
	let mem_space = mem_space_mutex.lock();
	let path = pathname.get(&mem_space)?.ok_or(errno!(EFAULT))?;
	let path = Path::new(path)?;

	let rs = ResolutionSettings::for_process(&proc, true);

	// Remove the file
	vfs::remove_file_from_path(path, &rs)?;

	Ok(0)
}

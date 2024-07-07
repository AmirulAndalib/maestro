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

//! The `_llseek` system call repositions the offset of a file descriptor.

use crate::{
	file::fd::FileDescriptorTable,
	process::{mem_space::MemSpace, Process},
	syscall::{Args, SyscallPtr},
};
use core::ffi::{c_uint, c_ulong};
use utils::{
	errno,
	errno::{EResult, Errno},
	io::IO,
	lock::{IntMutex, Mutex},
	ptr::arc::Arc,
};

/// Sets the offset from the given value.
const SEEK_SET: u32 = 0;
/// Sets the offset relative to the current offset.
const SEEK_CUR: u32 = 1;
/// Sets the offset relative to the end of the file.
const SEEK_END: u32 = 2;

pub fn _llseek(
	Args((fd, offset_high, offset_low, result, whence)): Args<(
		c_uint,
		c_ulong,
		c_ulong,
		SyscallPtr<u64>,
		c_uint,
	)>,
	fds_mutex: Arc<Mutex<FileDescriptorTable>>,
	mem_space_mutex: Arc<IntMutex<MemSpace>>,
) -> EResult<usize> {
	let fds = fds_mutex.lock();
	let open_file_mutex = fds.get_fd(fd as _)?.get_open_file();
	// Get file
	let mut open_file = open_file_mutex.lock();
	// Compute the offset
	let off = ((offset_high as u64) << 32) | (offset_low as u64);
	let off = match whence {
		SEEK_SET => off,
		SEEK_CUR => open_file
			.get_offset()
			.checked_add(off)
			.ok_or_else(|| errno!(EOVERFLOW))?,
		SEEK_END => open_file
			.get_size()
			.checked_add(off)
			.ok_or_else(|| errno!(EOVERFLOW))?,
		_ => return Err(errno!(EINVAL)),
	};
	{
		let mut mem_space = mem_space_mutex.lock();
		// Write the result to the userspace
		if let Some(result) = result.get_mut(&mut mem_space)? {
			*result = off;
		}
	}
	// Set the new offset
	open_file.set_offset(off);
	Ok(0)
}

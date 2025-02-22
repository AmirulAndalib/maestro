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

//! The `mmap` system call allows the process to allocate memory.

use crate::{
	file::{fd::FileDescriptorTable, perm::AccessProfile, FileType},
	memory,
	memory::VirtAddr,
	process::{mem_space, mem_space::MemSpace, Process},
	sync::mutex::{IntMutex, Mutex},
	syscall::{mmap::mem_space::MapConstraint, Args},
};
use core::{
	ffi::{c_int, c_void},
	intrinsics::unlikely,
	num::NonZeroUsize,
};
use utils::{
	errno,
	errno::{EResult, Errno},
	limits::PAGE_SIZE,
	ptr::arc::Arc,
};

/// Data can be read.
pub const PROT_READ: i32 = 0b001;
/// Data can be written.
pub const PROT_WRITE: i32 = 0b010;
/// Data can be executed.
pub const PROT_EXEC: i32 = 0b100;

/// Changes are shared.
const MAP_SHARED: i32 = 0b001;
/// Interpret addr exactly.
const MAP_FIXED: i32 = 0b010;

/// Converts mmap's `flags` and `prot` to mem space mapping flags.
fn get_flags(flags: i32, prot: i32) -> u8 {
	let mut mem_flags = mem_space::MAPPING_FLAG_USER;
	if flags & MAP_SHARED != 0 {
		mem_flags |= mem_space::MAPPING_FLAG_SHARED;
	}
	if prot & PROT_WRITE != 0 {
		mem_flags |= mem_space::MAPPING_FLAG_WRITE;
	}
	if prot & PROT_EXEC != 0 {
		mem_flags |= mem_space::MAPPING_FLAG_EXEC;
	}
	mem_flags
}

/// Performs the `mmap` system call.
#[allow(clippy::too_many_arguments)]
pub fn do_mmap(
	addr: VirtAddr,
	length: usize,
	prot: i32,
	flags: i32,
	fd: i32,
	offset: u64,
	fds: Arc<Mutex<FileDescriptorTable>>,
	ap: AccessProfile,
	mem_space: Arc<IntMutex<MemSpace>>,
) -> EResult<usize> {
	// Check alignment of `addr` and `length`
	if !addr.is_aligned_to(PAGE_SIZE) || length == 0 {
		return Err(errno!(EINVAL));
	}
	// The length in number of pages
	let pages = length.div_ceil(PAGE_SIZE);
	let Some(pages) = NonZeroUsize::new(pages) else {
		return Err(errno!(EINVAL));
	};
	// Check for overflow
	if unlikely(addr.0.checked_add(pages.get() * PAGE_SIZE).is_none()) {
		return Err(errno!(EINVAL));
	}
	let constraint = {
		if !addr.is_null() {
			if flags & MAP_FIXED != 0 {
				MapConstraint::Fixed(addr)
			} else {
				MapConstraint::Hint(addr)
			}
		} else {
			MapConstraint::None
		}
	};
	let file = if fd >= 0 {
		// Check the alignment of the offset
		if offset as usize % PAGE_SIZE != 0 {
			return Err(errno!(EINVAL));
		}
		let fd = fds.lock().get_fd(fd)?.get_file().clone();
		Some(fd)
	} else {
		None
	};
	// TODO anon flag
	if let Some(file) = &file {
		let stat = file.stat()?;
		// Check the file is suitable
		if stat.get_type() != Some(FileType::Regular) {
			return Err(errno!(EACCES));
		}
		if prot & PROT_READ != 0 && !ap.can_read_file(&stat) {
			return Err(errno!(EPERM));
		}
		if prot & PROT_WRITE != 0 && !ap.can_write_file(&stat) {
			return Err(errno!(EPERM));
		}
		if prot & PROT_EXEC != 0 && !ap.can_execute_file(&stat) {
			return Err(errno!(EPERM));
		}
	} else {
		// TODO If the mapping requires a fd, return an error
	}
	let flags = get_flags(flags, prot);
	let mut mem_space = mem_space.lock();
	// The pointer on the virtual memory to the beginning of the mapping
	let result = mem_space.map(constraint, pages, flags, file.clone(), offset);
	match result {
		Ok(ptr) => Ok(ptr as _),
		Err(e) => {
			if constraint != MapConstraint::None {
				let ptr = mem_space.map(MapConstraint::None, pages, flags, file, offset)?;
				Ok(ptr as _)
			} else {
				Err(e.into())
			}
		}
	}
}

pub fn mmap(
	Args((addr, length, prot, flags, fd, offset)): Args<(
		VirtAddr,
		usize,
		c_int,
		c_int,
		c_int,
		u64,
	)>,
	fds: Arc<Mutex<FileDescriptorTable>>,
	ap: AccessProfile,
	mem_space: Arc<IntMutex<MemSpace>>,
) -> EResult<usize> {
	do_mmap(
		addr,
		length,
		prot,
		flags,
		fd,
		offset as _,
		fds,
		ap,
		mem_space,
	)
}

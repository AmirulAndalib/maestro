//! The `mmap` system call allows the process to allocate memory.

use core::ffi::c_int;
use core::ffi::c_void;
use crate::errno::Errno;
use crate::errno;
use crate::file::FileType;
use crate::memory;
use crate::process::Process;
use crate::process::mem_space::MapResidence;
use crate::process::mem_space;
use crate::syscall::mmap::mem_space::MapConstraint;
use crate::util::math;
use crate::util;
use macros::syscall;

/// Data can be read.
const PROT_READ: i32 = 0b001;
/// Data can be written.
const PROT_WRITE: i32 = 0b010;
/// Data can be executed.
const PROT_EXEC: i32 = 0b100;

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
/// This function takes a `u64` for `offset` to allow implementing the `mmap2`
/// syscall.
pub fn do_mmap(
	addr: *mut c_void,
	length: usize,
	prot: i32,
	flags: i32,
	fd: i32,
	offset: u64,
) -> Result<i32, Errno> {
	// Checking alignment of `addr` and `length`
	if !util::is_aligned(addr, memory::PAGE_SIZE) || length == 0 {
		return Err(errno!(EINVAL));
	}

	// The length in number of pages
	let pages = math::ceil_division(length, memory::PAGE_SIZE);

	// Checking for overflow
	let end = (addr as usize).wrapping_add(pages * memory::PAGE_SIZE);
	if end < addr as usize {
		return Err(errno!(EINVAL));
	}

	let addr_hint = {
		if !addr.is_null()
			&& (addr as usize) < (memory::PROCESS_END as usize)
			&& end <= (memory::PROCESS_END as usize)
		{
			MapConstraint::Hint(addr as *const c_void)
		} else {
			MapConstraint::None
		}
	};

	// Getting the current process
	let mutex = Process::get_current().unwrap();
	let guard = mutex.lock();
	let proc = guard.get_mut();

	let uid = proc.get_euid();
	let gid = proc.get_egid();

	// The file the mapping points to
	let open_file_mutex = if fd >= 0 {
		let fds_mutex = proc.get_fds().unwrap();
		let fds_guard = fds_mutex.lock();
		let fds = fds_guard.get();

		fds.get_fd(fd as _)
			.map(|fd| fd.get_open_file())
			.transpose()?
	} else {
		None
	};

	// TODO anon flag

	if let Some(open_file_mutex) = &open_file_mutex {
		// Checking the alignment of the offset
		if offset as usize % memory::PAGE_SIZE != 0 {
			return Err(errno!(EINVAL));
		}

		let open_file_guard = open_file_mutex.lock();
		let open_file = open_file_guard.get();

		let file_mutex = open_file.get_file()?;
		let file_guard = file_mutex.lock();
		let file = file_guard.get();

		if !matches!(file.get_type(), FileType::Regular) {
			return Err(errno!(EACCES));
		}

		if prot & PROT_READ != 0 && !file.can_read(uid, gid) {
			return Err(errno!(EPERM));
		}
		if prot & PROT_WRITE != 0 && !file.can_write(uid, gid) {
			return Err(errno!(EPERM));
		}
		// TODO check exec
	} else {
		// TODO If the mapping requires a fd, return an error
	}
	let residence = match open_file_mutex {
		Some(file) => MapResidence::File {
			file,
			off: offset,
		},

		None => MapResidence::Normal,
	};

	// The process's memory space
	let mem_space = proc.get_mem_space().unwrap();
	let mem_space_guard = mem_space.lock();
	let mem_space = mem_space_guard.get_mut();

	let flags = get_flags(flags, prot);

	// The pointer on the virtual memory to the beginning of the mapping
	let result = mem_space.map(
		addr_hint,
		pages,
		flags,
		residence.clone(),
	);

	match result {
		Ok(ptr) => Ok(ptr),

		Err(e) => {
			if addr_hint != MapConstraint::None {
				mem_space.map(
					MapConstraint::None,
					pages,
					flags,
					residence,
				)
			} else {
				Err(e)
			}
		},
	}.map(|ptr| ptr as _)
}

// TODO Check last arg type
/// The implementation of the `mmap` syscall.
#[syscall]
pub fn mmap(
	addr: *mut c_void,
	length: usize,
	prot: c_int,
	flags: c_int,
	fd: c_int,
	offset: u64,
) -> Result<i32, Errno> {
	do_mmap(addr, length, prot, flags, fd, offset as _)
}

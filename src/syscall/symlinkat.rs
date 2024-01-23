//! The `symlinkat` syscall allows to create a symbolic link.

use super::util;
use crate::errno::Errno;
use crate::file::path::PathBuf;
use crate::file::FileContent;
use crate::limits;
use crate::process::mem_space::ptr::SyscallString;
use crate::process::Process;
use core::ffi::c_int;
use macros::syscall;

#[syscall]
pub fn symlinkat(
	target: SyscallString,
	newdirfd: c_int,
	linkpath: SyscallString,
) -> Result<i32, Errno> {
	let proc_mutex = Process::current_assert();
	let proc = proc_mutex.lock();

	let mem_space = proc.get_mem_space().unwrap().clone();
	let mem_space_guard = mem_space.lock();

	let target_slice = target
		.get(&mem_space_guard)?
		.ok_or_else(|| errno!(EFAULT))?;
	if target_slice.len() > limits::SYMLINK_MAX {
		return Err(errno!(ENAMETOOLONG));
	}
	let target = PathBuf::try_from(target_slice)?;
	let file_content = FileContent::Link(target);

	let linkpath = linkpath
		.get(&mem_space_guard)?
		.ok_or_else(|| errno!(EFAULT))?;
	util::create_file_at(proc, newdirfd, linkpath, 0, file_content, true, 0)?;

	Ok(0)
}

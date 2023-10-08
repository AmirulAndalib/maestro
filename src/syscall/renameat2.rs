//! The `renameat2` allows to rename a file.

use crate::errno::Errno;
use crate::file;
use crate::file::vfs;
use crate::file::FileType;
use crate::process::mem_space::ptr::SyscallString;
use crate::process::Process;
use core::ffi::c_int;
use macros::syscall;

/// Flag: Don't replace new path if it exists. Return an error instead.
const RENAME_NOREPLACE: c_int = 1;
/// Flag: Exchanges old and new paths atomically.
const RENAME_EXCHANGE: c_int = 2;

#[syscall]
pub fn renameat2(
	olddirfd: c_int,
	oldpath: SyscallString,
	newdirfd: c_int,
	newpath: SyscallString,
	_flags: c_int,
) -> Result<i32, Errno> {
	let (old_mutex, new_parent_mutex, new_name, ap) = {
		let proc_mutex = Process::current_assert();
		let proc = proc_mutex.lock();

		let ap = proc.access_profile;

		let mem_space = proc.get_mem_space().unwrap().clone();
		let mem_space_guard = mem_space.lock();

		let oldpath = oldpath
			.get(&mem_space_guard)?
			.ok_or_else(|| errno!(EFAULT))?;
		let old = super::util::get_file_at(proc, false, olddirfd, oldpath, 0)?;

		let proc = proc_mutex.lock();
		let newpath = newpath
			.get(&mem_space_guard)?
			.ok_or_else(|| errno!(EFAULT))?;
		let (new_parent, new_name) =
			super::util::get_parent_at_with_name(proc, false, newdirfd, newpath)?;

		(old, new_parent, new_name, ap)
	};

	let mut old = old_mutex.lock();
	let mut new_parent = new_parent_mutex.lock();

	// TODO Check permissions if sticky bit is set

	if new_parent.get_location().get_mountpoint_id() == old.get_location().get_mountpoint_id() {
		// Old and new are both on the same filesystem

		// TODO On fail, undo

		// Create link at new location
		// The `..` entry is already updated by the file system since having the same
		// directory in several locations is not allowed
		vfs::create_link(&mut old, &mut new_parent, &new_name, &ap)?;

		if old.get_type() != FileType::Directory {
			vfs::remove_file(&mut old, &ap)?;
		}
	} else {
		// Old and new are on different filesystems.

		// TODO On fail, undo

		file::util::copy_file(&mut old, &mut new_parent, new_name)?;
		file::util::remove_recursive(&mut old, &ap)?;
	}

	Ok(0)
}

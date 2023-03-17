//! The `renameat2` allows to rename a file.

use core::ffi::c_int;
use crate::errno::Errno;
use crate::file::FileType;
use crate::file::vfs;
use crate::file;
use crate::process::Process;
use crate::process::mem_space::ptr::SyscallString;
use macros::syscall;

/// Flag: Don't replace new path if it exists. Return an error instead.
const RENAME_NOREPLACE: c_int = 1;
/// Flag: Exchanges old and new paths atomically.
const RENAME_EXCHANGE: c_int = 2;
/// TODO doc
const RENAME_WHITEOUT: c_int = 4;

#[syscall]
pub fn renameat2(
	olddirfd: c_int,
	oldpath: SyscallString,
	newdirfd: c_int,
	newpath: SyscallString,
	_flags: c_int,
) -> Result<i32, Errno> {
	let (uid, gid, old_mutex, new_parent_mutex, new_name) = {
		let proc_mutex = Process::get_current().unwrap();
		let proc = proc_mutex.lock();

		let uid = proc.get_euid();
		let gid = proc.get_egid();

		let mem_space = proc.get_mem_space().clone().unwrap();
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

		(uid, gid, old, new_parent, new_name)
	};

	let mut old = old_mutex.lock();
	let mut new_parent = new_parent_mutex.lock();

	// TODO Check permissions if sticky bit is set

	let vfs = vfs::get();
	let mut vfs = vfs.lock();
	let vfs = vfs.as_mut().unwrap();

	if new_parent.get_location().get_mountpoint_id() == old.get_location().get_mountpoint_id() {
		// Old and new are both on the same filesystem

		// TODO On fail, undo

		// Create link at new location
		// The `..` entry is already updated by the file system since having the same
		// directory in several locations is not allowed
		vfs.create_link(&mut *old, &mut *new_parent, &new_name, uid, gid)?;

		if old.get_type() != FileType::Directory {
			vfs.remove_file(&*old, uid, gid)?;
		}
	} else {
		// Old and new are on different filesystems.

		// TODO On fail, undo

		file::util::copy_file(vfs, &mut *old, &mut *new_parent, new_name)?;
		file::util::remove_recursive(vfs, &mut *old, uid, gid)?;
	}

	Ok(0)
}

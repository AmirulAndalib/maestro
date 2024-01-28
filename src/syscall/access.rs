//! The `access` system call allows to check access to a given file.

use crate::{
	errno::Errno,
	file::{
		path::Path,
		vfs::{ResolutionSettings, Resolved},
	},
	process::{mem_space::ptr::SyscallString, Process},
	syscall::util::{
		at,
		at::{AT_EACCESS, AT_FDCWD},
	},
};
use core::ffi::c_int;
use macros::syscall;

/// Checks for existence of the file.
const F_OK: i32 = 0;
/// Checks the file can be read.
const R_OK: i32 = 4;
/// Checks the file can be written.
const W_OK: i32 = 2;
/// Checks the file can be executed.
const X_OK: i32 = 1;

/// Performs the access operation.
///
/// Arguments:
/// - `dirfd` is the file descriptor of the directory relative to which the check
/// is done.
/// - `pathname` is the path to the file.
/// - `mode` is a bitfield of access permissions to check.
/// - `flags` is a set of flags.
pub fn do_access(
	dirfd: Option<i32>,
	pathname: SyscallString,
	mode: i32,
	flags: Option<i32>,
) -> Result<i32, Errno> {
	let flags = flags.unwrap_or(0);
	// Use effective IDs instead of real IDs
	let eaccess = flags & AT_EACCESS != 0;

	let (file, ap) = {
		let proc_mutex = Process::current_assert();
		let proc = proc_mutex.lock();

		let rs = ResolutionSettings::for_process(&proc, true);

		let mem_space_mutex = proc.get_mem_space().unwrap();
		let mem_space_guard = mem_space_mutex.lock();

		let fds = proc.file_descriptors.as_ref().unwrap().lock();

		let pathname = pathname
			.get(&mem_space_guard)?
			.ok_or_else(|| errno!(EINVAL))?;
		let path = Path::new(pathname)?;

		let Resolved::Found(file) =
			at::get_file(&fds, rs, dirfd.unwrap_or(AT_FDCWD), path, flags)?
		else {
			return Err(errno!(ENOENT));
		};

		(file, proc.access_profile)
	};

	// Do access checks
	{
		let file = file.lock();
		if (mode & R_OK != 0) && !ap.check_read_access(&file, eaccess) {
			return Err(errno!(EACCES));
		}
		if (mode & W_OK != 0) && !ap.check_write_access(&file, eaccess) {
			return Err(errno!(EACCES));
		}
		if (mode & X_OK != 0) && !ap.check_execute_access(&file, eaccess) {
			return Err(errno!(EACCES));
		}
	}

	Ok(0)
}

#[syscall]
pub fn access(pathname: SyscallString, mode: c_int) -> Result<i32, Errno> {
	do_access(None, pathname, mode, None)
}

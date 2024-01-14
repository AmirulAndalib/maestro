//! The fchdir system call allows to change the current working directory of the
//! current process.

use crate::errno;
use crate::errno::Errno;
use crate::file::FileType;
use crate::process::Process;
use crate::util::ptr::arc::Arc;
use core::ffi::c_int;
use macros::syscall;

#[syscall]
pub fn fchdir(fd: c_int) -> Result<i32, Errno> {
	if fd < 0 {
		return Err(errno!(EBADF));
	}

	let (open_file_mutex, ap) = {
		let proc_mutex = Process::current_assert();
		let proc = proc_mutex.lock();

		let fds_mutex = proc.file_descriptors.as_ref().unwrap();
		let fds = fds_mutex.lock();

		let open_file_mutex = fds
			.get_fd(fd as _)
			.ok_or_else(|| errno!(EBADF))?
			.get_open_file()
			.clone();

		(open_file_mutex, proc.access_profile)
	};
	let open_file = open_file_mutex.lock();

	let new_cwd = {
		let file = open_file.get_file().lock();

		// Check for errors
		if file.get_type() != FileType::Directory {
			return Err(errno!(ENOTDIR));
		}
		if !ap.can_list_directory(&file) {
			return Err(errno!(EACCES));
		}

		file.get_path()
	}?;

	{
		let proc_mutex = Process::current_assert();
		let mut proc = proc_mutex.lock();

		let new_cwd = super::util::get_absolute_path(&proc, new_cwd)?;
		proc.cwd = Arc::new(new_cwd)?;
	}

	Ok(0)
}

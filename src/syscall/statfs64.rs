//! The `statfs64` system call returns information about a mounted file system.

use crate::errno;
use crate::errno::Errno;
use crate::file::fs::Statfs;
use crate::file::path::Path;
use crate::file::vfs;
use crate::process::mem_space::ptr::SyscallPtr;
use crate::process::mem_space::ptr::SyscallString;
use crate::process::Process;
use macros::syscall;

// TODO Streamline with `[f]statfs`

// TODO Check args types
#[syscall]
pub fn statfs64(path: SyscallString, _sz: usize, buf: SyscallPtr<Statfs>) -> Result<i32, Errno> {
	// TODO Use `sz`

	let (path, ap) = {
		let proc_mutex = Process::current_assert();
		let proc = proc_mutex.lock();

		let mem_space = proc.get_mem_space().unwrap();
		let mem_space_guard = mem_space.lock();

		let path = path.get(&mem_space_guard)?.ok_or_else(|| errno!(EFAULT))?;
		let path = Path::new(path)?;
		let path = super::util::get_absolute_path(&proc, path)?;

		(path, proc.access_profile)
	};

	let file_mutex = vfs::get_file_from_path(&path, &ap, true)?;
	let file = file_mutex.lock();

	let mountpoint_mutex = file.get_location().get_mountpoint().unwrap();
	let mountpoint = mountpoint_mutex.lock();

	let io_mutex = mountpoint.get_source().get_io()?;
	let mut io = io_mutex.lock();

	let fs_mutex = mountpoint.get_filesystem();
	let fs = fs_mutex.lock();

	let stat = fs.get_stat(&mut *io)?;

	// Writing the statfs structure to userspace
	{
		let proc_mutex = Process::current_assert();
		let proc = proc_mutex.lock();

		let mem_space = proc.get_mem_space().unwrap();
		let mut mem_space_guard = mem_space.lock();

		let buf = buf
			.get_mut(&mut mem_space_guard)?
			.ok_or_else(|| errno!(EFAULT))?;
		*buf = stat;
	}

	Ok(0)
}

//! This module implements utility functions for system calls.

use crate::errno;
use crate::errno::AllocResult;
use crate::errno::EResult;
use crate::file::path::Path;
use crate::file::vfs;
use crate::file::File;
use crate::file::FileContent;
use crate::file::Mode;
use crate::process::mem_space::ptr::SyscallString;
use crate::process::regs::Regs;
use crate::process::scheduler;
use crate::process::Process;
use crate::process::State;
use crate::util::container::string::String;
use crate::util::container::vec::Vec;
use crate::util::lock::Mutex;
use crate::util::lock::MutexGuard;
use crate::util::ptr::arc::Arc;
use core::mem::size_of;

/// Returns the absolute path according to the process's current working
/// directory.
///
/// Arguments:
/// - `process` is the process.
/// - `path` is the path.
pub fn get_absolute_path(process: &Process, path: Path) -> AllocResult<Path> {
	// TODO use chain + collect to allocate once
	let path = if !path.is_absolute() {
		process.cwd.concat(&path)?
	} else {
		path
	};
	process.chroot.concat(&path)
}

// TODO Find a safer and cleaner solution
/// Checks that the given array of strings at pointer `ptr` is accessible to
/// process `proc`, then returns its content.
///
/// If the array or its content strings are not accessible by the process, the
/// function returns an error.
pub unsafe fn get_str_array(process: &Process, ptr: *const *const u8) -> EResult<Vec<String>> {
	let mem_space = process.get_mem_space().unwrap();
	let mem_space_guard = mem_space.lock();

	// Checking every elements of the array and counting the number of elements
	let mut len = 0;
	loop {
		let elem_ptr = ptr.add(len);

		// Checking access on elem_ptr
		if !mem_space_guard.can_access(elem_ptr as _, size_of::<*const u8>(), true, false) {
			return Err(errno!(EFAULT));
		}

		// Safe because the access is checked before
		let elem = *elem_ptr;
		if elem.is_null() {
			break;
		}

		len += 1;
	}

	// Filling the array
	// TODO collect
	let mut arr = Vec::with_capacity(len)?;
	for i in 0..len {
		let elem = *ptr.add(i);
		let s: SyscallString = (elem as usize).into();

		arr.push(String::try_from(s.get(&mem_space_guard)?.unwrap())?)?;
	}

	Ok(arr)
}

/// Builds a path with the given directory file descriptor `dirfd` as a base,
/// concatenated with the given pathname `pathname`.
///
/// `process_guard` is the guard of the current process.
fn build_path_from_fd(
	process: MutexGuard<Process, false>,
	dirfd: i32,
	pathname: &[u8],
) -> EResult<Path> {
	let path = Path::from_str(pathname, true)?;

	if path.is_absolute() {
		// Using the given absolute path
		Ok(path)
	} else if dirfd == super::access::AT_FDCWD {
		// Using path relative to the current working directory
		Ok(process.cwd.concat(&path)?)
	} else {
		// Using path relative to the directory given by `dirfd`

		if dirfd < 0 {
			return Err(errno!(EBADF));
		}
		let fds_mutex = process.get_fds().unwrap();
		let fds = fds_mutex.lock();
		let open_file_mutex = fds
			.get_fd(dirfd as _)
			.ok_or(errno!(EBADF))?
			.get_open_file()
			.clone();
		drop(fds);

		// Unlock to avoid deadlock with procfs
		drop(process);

		let open_file = open_file_mutex.lock();
		let file_mutex = open_file.get_file();
		let file = file_mutex.lock();
		Ok(file.get_path()?.concat(&path)?)
	}
}

/// Returns the file for the given path `pathname`.
///
/// This function is useful for system calls with the `at` prefix.
///
/// Arguments:
/// - `process` is the mutex guard of the current process.
/// - `dirfd` is the file descriptor of the parent directory.
/// - `pathname` is the path relative to the parent directory.
/// - `follow_links_default` tells whether symbolic links may be followed if no flag is specified
///   about it.
/// - `flags` is an integer containing `AT_*` flags.
pub fn get_file_at(
	process: MutexGuard<Process, false>,
	dirfd: i32,
	pathname: &[u8],
	follow_links_default: bool,
	flags: i32,
) -> EResult<Arc<Mutex<File>>> {
	let follow_links = if follow_links_default {
		flags & super::access::AT_SYMLINK_NOFOLLOW != 0
	} else {
		flags & super::access::AT_SYMLINK_FOLLOW != 0
	};
	if pathname.is_empty() {
		if flags & super::access::AT_EMPTY_PATH != 0 {
			// Using `dirfd` as the file descriptor to the file

			if dirfd < 0 {
				return Err(errno!(EBADF));
			}
			let fds_mutex = process.get_fds().unwrap();
			let fds = fds_mutex.lock();
			let open_file_mutex = fds
				.get_fd(dirfd as _)
				.ok_or(errno!(EBADF))?
				.get_open_file()
				.clone();
			drop(fds);

			// Unlocking to avoid deadlock with procfs
			drop(process);

			let open_file = open_file_mutex.lock();
			Ok(open_file.get_file().clone())
		} else {
			Err(errno!(ENOENT))
		}
	} else {
		let ap = process.access_profile;
		let path = build_path_from_fd(process, dirfd, pathname)?;
		vfs::get_file_from_path(&path, &ap, follow_links)
	}
}

/// Returns the parent directory of the file for the given path `pathname`.
///
/// This function is useful for system calls with the `at` prefix.
///
/// Arguments:
/// - `process` is the mutex guard of the current process.
/// - `dirfd` is the file descriptor of the parent directory.
/// - `pathname` is the path relative to the parent directory.
/// - `follow_links_default` tells whether symbolic links may be followed if no flag is specified
///   about it.
/// - `flags` is an integer containing `AT_*` flags.
pub fn get_parent_at_with_name(
	process: MutexGuard<Process, false>,
	dirfd: i32,
	pathname: &[u8],
	follow_links_default: bool,
	flags: i32,
) -> EResult<(Arc<Mutex<File>>, String)> {
	let follow_links = if follow_links_default {
		flags & super::access::AT_SYMLINK_NOFOLLOW != 0
	} else {
		flags & super::access::AT_SYMLINK_FOLLOW != 0
	};
	let ap = process.access_profile;

	if pathname.is_empty() {
		return Err(errno!(ENOENT));
	}
	let mut path = build_path_from_fd(process, dirfd, pathname)?;
	let name = path.pop().unwrap();

	let parent_mutex = vfs::get_file_from_path(&path, &ap, follow_links)?;
	Ok((parent_mutex, name))
}

/// Creates the given file `file` at the given pathname `pathname`.
///
/// Arguments:
/// - `process` is the mutex guard of the current process.
/// - `dirfd` is the file descriptor of the parent directory.
/// - `pathname` is the path relative to the parent directory.
/// - `mode` is the permissions of the newly created file.
/// - `content` is the content of the newly created file.
/// - `follow_links_default` tells whether symbolic links may be followed if no flag is specified
///   about it.
/// - `flags` is an integer containing `AT_*` flags.
pub fn create_file_at(
	process: MutexGuard<Process, false>,
	dirfd: i32,
	pathname: &[u8],
	mode: Mode,
	content: FileContent,
	follow_links_default: bool,
	flags: i32,
) -> EResult<Arc<Mutex<File>>> {
	let ap = process.access_profile;
	let mode = mode & !process.umask;

	let (parent_mutex, name) =
		get_parent_at_with_name(process, dirfd, pathname, follow_links_default, flags)?;

	let mut parent = parent_mutex.lock();
	vfs::create_file(&mut parent, name, &ap, mode, content)
}

/// Updates the execution flow of the current process according to its state.
///
/// When the state of the current process has been changed, execution may not
/// resume. In which case, the current function handles the execution flow
/// accordingly.
///
/// The functions locks the mutex of the current process. Thus, the caller must
/// ensure the mutex isn't already locked to prevent a deadlock.
///
/// If returning, the function returns the mutex lock of the current process.
pub fn handle_proc_state() {
	let proc_mutex = Process::current_assert();
	let proc = proc_mutex.lock();

	match proc.get_state() {
		// The process is executing a signal handler. Make the scheduler jump to it
		State::Running => {
			if proc.is_handling_signal() {
				let regs = proc.regs.clone();
				drop(proc);
				drop(proc_mutex);

				unsafe {
					regs.switch(true);
				}
			}
		}

		// The process is sleeping or has been stopped. Waiting until wakeup
		State::Sleeping | State::Stopped => {
			drop(proc);
			drop(proc_mutex);

			scheduler::end_tick();
		}

		// The process has been killed. Stopping execution and waiting for the next tick
		State::Zombie => {
			drop(proc);
			drop(proc_mutex);

			scheduler::end_tick();
		}
	}
}

/// Checks whether the current syscall must be interrupted to execute a signal.
///
/// If interrupted, the function doesn't return and the control flow jumps
/// directly to handling the signal.
///
/// The functions locks the mutex of the current process. Thus, the caller must
/// ensure the mutex isn't already locked to prevent a deadlock.
///
/// `regs` is the registers state passed to the current syscall.
pub fn signal_check(regs: &Regs) {
	let proc_mutex = Process::current_assert();
	let mut proc = proc_mutex.lock();

	if proc.get_next_signal().is_some() {
		// Returning the system call early to resume it later
		let mut r = regs.clone();
		// TODO Clean
		r.eip -= 2; // TODO Handle the case where the instruction insn't two bytes long (sysenter)
		proc.regs = r;
		proc.syscalling = false;

		// Switching to handle the signal
		proc.prepare_switch();

		drop(proc);
		drop(proc_mutex);

		handle_proc_state();
	}
}

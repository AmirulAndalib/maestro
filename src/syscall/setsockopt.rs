//! The `setsockopt` system call sets an option on a socket.

use crate::errno::Errno;
use crate::file::buffer;
use crate::file::buffer::socket::Socket;
use crate::process::mem_space::ptr::SyscallSlice;
use crate::process::Process;
use core::any::Any;
use core::ffi::c_int;
use macros::syscall;

#[syscall]
pub fn setsockopt(
	sockfd: c_int,
	level: c_int,
	optname: c_int,
	optval: SyscallSlice<u8>,
	optlen: usize,
) -> Result<i32, Errno> {
	if sockfd < 0 {
		return Err(errno!(EBADF));
	}

	let proc_mutex = Process::get_current().unwrap();
	let proc = proc_mutex.lock();

	// Get socket
	let fds_mutex = proc.get_fds().unwrap();
	let fds = fds_mutex.lock();
	let fd = fds.get_fd(sockfd as _).ok_or_else(|| errno!(EBADF))?;
	let open_file_mutex = fd.get_open_file()?;
	let open_file = open_file_mutex.lock();
	let loc = open_file.get_location();
	let sock_mutex = buffer::get_or_default::<Socket>(loc)?;
	let mut sock = sock_mutex.lock();
	let sock = (&mut *sock as &mut dyn Any)
		.downcast_mut::<Socket>()
		.unwrap();

	// Get optval slice
	let mem_space = proc.get_mem_space().unwrap();
	let mut mem_space_guard = mem_space.lock();
	let optval_slice = optval
		.get(&mut mem_space_guard, optlen)?
		.ok_or(errno!(EFAULT))?;

	sock.set_opt(level, optname, optval_slice)
}

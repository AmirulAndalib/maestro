//! The `socket` system call allows to create a socket.

use crate::errno;
use crate::errno::Errno;
use crate::file::buffer;
use crate::file::buffer::socket::Socket;
use crate::file::open_file;
use crate::file::open_file::OpenFile;
use crate::net::SocketDesc;
use crate::net::SocketDomain;
use crate::net::SocketType;
use crate::process::Process;
use core::ffi::c_int;
use macros::syscall;

/// The implementation of the `socket` syscall.
#[syscall]
pub fn socket(domain: c_int, r#type: c_int, protocol: c_int) -> Result<i32, Errno> {
	let proc_mutex = Process::current_assert();
	let proc = proc_mutex.lock();

	let sock_domain = SocketDomain::try_from(domain as u32)?;
	let sock_type = SocketType::try_from(r#type as u32)?;
	if !proc.access_profile.can_use_sock_domain(&sock_domain)
		|| !proc.access_profile.can_use_sock_type(&sock_type)
	{
		return Err(errno!(EACCES));
	}
	let desc = SocketDesc {
		domain: sock_domain,
		type_: sock_type,
		protocol,
	};

	let sock = Socket::new(desc)?;
	let loc = buffer::register(None, sock)?;

	let fds_mutex = proc.get_fds().unwrap();
	let mut fds = fds_mutex.lock();

	let open_file = OpenFile::new(loc.clone(), open_file::O_RDWR);
	let sock_fd = fds.create_fd(0, open_file)?;

	Ok(sock_fd.get_id() as _)
}

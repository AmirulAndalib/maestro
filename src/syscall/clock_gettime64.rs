//! `clock_gettime64` is like `clock_gettime` but using 64 bits.

use crate::errno::Errno;
use crate::process::mem_space::ptr::SyscallPtr;
use crate::process::Process;
use crate::time;
use crate::time::unit::ClockIdT;
use crate::time::unit::Timespec;
use macros::syscall;

// TODO Check first arg type
#[syscall]
pub fn clock_gettime64(_clockid: ClockIdT, tp: SyscallPtr<Timespec>) -> Result<i32, Errno> {
	// TODO Get clock according to param
	let clk = b"TODO";
	let curr_time = time::get_struct::<Timespec>(clk, true).ok_or(errno!(EINVAL))?;

	{
		let proc_mutex = Process::get_current().unwrap();
		let proc = proc_mutex.lock();

		let mem_space = proc.get_mem_space().unwrap();
		let mut mem_space_guard = mem_space.lock();
		let timespec = tp.get_mut(&mut mem_space_guard)?.ok_or(errno!(EFAULT))?;

		*timespec = curr_time;
	}

	Ok(0)
}

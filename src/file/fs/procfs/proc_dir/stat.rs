//! This module implements the stat file, which allows to retrieve the current
//! status of the process.

use crate::errno::Errno;
use crate::file::fs::kernfs::node::KernFSNode;
use crate::file::FileContent;
use crate::file::Gid;
use crate::file::Mode;
use crate::file::Uid;
use crate::process::pid::Pid;
use crate::process::Process;
use crate::util::io::IO;
use crate::util::ptr::cow::Cow;
use core::cmp::min;

/// Structure representing the stat node of the procfs.
pub struct Stat {
	/// The PID of the process.
	pub pid: Pid,
}

impl KernFSNode for Stat {
	fn get_mode(&self) -> Mode {
		0o444
	}

	fn get_uid(&self) -> Uid {
		if let Some(proc_mutex) = Process::get_by_pid(self.pid) {
			proc_mutex.lock().euid
		} else {
			0
		}
	}

	fn get_gid(&self) -> Gid {
		if let Some(proc_mutex) = Process::get_by_pid(self.pid) {
			proc_mutex.lock().egid
		} else {
			0
		}
	}

	fn get_content<'a>(&'a self) -> Cow<'a, FileContent> {
		Cow::from(FileContent::Regular)
	}
}

impl IO for Stat {
	fn get_size(&self) -> u64 {
		0
	}

	fn read(&mut self, offset: u64, buff: &mut [u8]) -> Result<(u64, bool), Errno> {
		if buff.is_empty() {
			return Ok((0, false));
		}

		let proc_mutex = Process::get_by_pid(self.pid).ok_or_else(|| errno!(ENOENT))?;
		let proc = proc_mutex.lock();

		let name = proc
			.argv
			.iter()
			.map(|name| unsafe { name.as_str_unchecked() })
			.next()
			.unwrap_or("?");

		let state = proc.get_state();
		let state_char = state.get_char();

		let pid = proc.pid;
		let ppid = proc.get_parent_pid();
		let pgid = proc.pgid;
		let sid = 0; // TODO

		let user_jiffies = 0; // TODO
		let kernel_jiffies = 0; // TODO

		let priority = proc.priority;
		let nice = proc.nice;

		let num_threads = 1; // TODO

		// TODO Fix deadlock
		//let vmem_usage = proc.get_vmem_usage();
		let vmem_usage = 0;

		let esp = proc.regs.esp;
		let eip = proc.regs.eip;

		// TODO Fill every fields with process's data
		// Generating content
		let content = crate::format!(
			"{pid} ({name}) {state_char} {ppid} {pgid} {sid} TODO TODO 0 \
0 0 0 0 {user_jiffies} {kernel_jiffies} TODO TODO {priority} {nice} {num_threads} 0 {vmem_usage} \
TODO TODO TODO TODO {esp} {eip} TODO TODO TODO TODO 0 0 0 TODO TODO TODO TODO TODO TODO TODO TODO \
TODO TODO TODO TODO TODO TODO TODO TODO TODO"
		)?;

		// Copying content to userspace buffer
		let content_bytes = content.as_bytes();
		let len = min((content_bytes.len() as u64 - offset) as usize, buff.len());
		buff[..len].copy_from_slice(&content_bytes[(offset as usize)..(offset as usize + len)]);

		let eof = (offset + len as u64) >= content_bytes.len() as u64;
		Ok((len as _, eof))
	}

	fn write(&mut self, _offset: u64, _buff: &[u8]) -> Result<u64, Errno> {
		Err(errno!(EINVAL))
	}

	fn poll(&mut self, _mask: u32) -> Result<u32, Errno> {
		// TODO
		todo!();
	}
}

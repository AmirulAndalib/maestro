/*
 * Copyright 2024 Luc Lenôtre
 *
 * This file is part of Maestro.
 *
 * Maestro is free software: you can redistribute it and/or modify it under the
 * terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or (at your option) any later
 * version.
 *
 * Maestro is distributed in the hope that it will be useful, but WITHOUT ANY
 * WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
 * A PARTICULAR PURPOSE. See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * Maestro. If not, see <https://www.gnu.org/licenses/>.
 */

//! The `kill` system call, which allows to send a signal to a process.

use super::{util, Args};
use crate::{
	process,
	process::{pid::Pid, regs::Regs, scheduler::SCHEDULER, signal::Signal, Process, State},
};
use core::ffi::c_int;
use utils::{
	errno,
	errno::{EResult, Errno},
	interrupt::cli,
};

/// Tries to kill the process with PID `pid` with the signal `sig`.
///
/// If `sig` is `None`, the function doesn't send a signal, but still checks if
/// there is a process that could be killed.
fn try_kill(pid: Pid, sig: Option<Signal>) -> EResult<()> {
	let proc_mutex = Process::current();
	let mut proc = proc_mutex.lock();
	let ap = proc.access_profile;
	// Closure sending the signal
	let f = |target: &mut Process| {
		if matches!(target.get_state(), State::Zombie) {
			return Ok(());
		}
		if !ap.can_kill(target) {
			return Err(errno!(EPERM));
		}
		if let Some(sig) = sig {
			target.kill(sig);
		}
		Ok(())
	};
	if pid == proc.get_pid() {
		f(&mut proc)?;
	} else {
		let target_mutex = Process::get_by_pid(pid).ok_or_else(|| errno!(ESRCH))?;
		let mut target_proc = target_mutex.lock();
		f(&mut target_proc)?;
	}
	Ok(())
}

/// Tries to kill a process group.
///
/// Arguments:
/// - `pid` is the value that determine which process(es) to kill.
/// - `sig` is the signal to send.
///
/// If `sig` is `None`, the function doesn't send a signal, but still checks if
/// there is a process that could be killed.
fn try_kill_group(pid: i32, sig: Option<Signal>) -> EResult<()> {
	let pgid = match pid {
		0 => {
			let proc_mutex = Process::current();
			let proc = proc_mutex.lock();
			proc.pgid
		}
		i if i < 0 => -pid as Pid,
		_ => pid as Pid,
	};
	// Kill process group
	Process::get_by_pid(pgid)
		.ok_or_else(|| errno!(ESRCH))?
		.lock()
		.get_group_processes()
		.iter()
		// Avoid deadlock
		.filter(|pid| **pid != pgid)
		.try_for_each(|pid| try_kill(*pid as _, sig))?;
	// Kill process group owner
	try_kill(pgid, sig)?;
	Ok(())
}

/// Sends the signal `sig` to the processes according to the given value `pid`.
///
/// If `sig` is `None`, the function doesn't send a signal, but still checks if
/// there is a process that could be killed.
fn send_signal(pid: i32, sig: Option<Signal>) -> EResult<()> {
	match pid {
		// Kill the process with the given PID
		1.. => try_kill(pid as _, sig),
		// Kill all processes in the current process group
		0 => try_kill_group(0, sig),
		// Kill all processes for which the current process has the permission
		-1 => {
			let sched = SCHEDULER.get().lock();
			for (pid, _) in sched.iter_process() {
				if *pid == process::pid::INIT_PID {
					continue;
				}
				// TODO Check permission
				try_kill(*pid, sig)?;
			}
			Ok(())
		}
		// Kill the given process group
		..-1 => try_kill_group(-pid as _, sig),
	}
}

pub fn kill(Args((pid, sig)): Args<(c_int, c_int)>) -> EResult<usize> {
	let sig = (sig != 0).then(|| Signal::try_from(sig)).transpose()?;
	send_signal(pid, sig)?;
	Ok(0)
}

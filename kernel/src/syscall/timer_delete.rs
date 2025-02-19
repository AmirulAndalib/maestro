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

//! The `timer_delete` system call deletes a per-process timer.

use crate::{process::Process, syscall::Args, time::unit::TimerT};
use utils::{
	errno::{EResult, Errno},
	ptr::arc::Arc,
};

pub fn timer_delete(Args(timerid): Args<TimerT>, proc: Arc<Process>) -> EResult<usize> {
	proc.timer_manager.lock().delete_timer(timerid)?;
	Ok(0)
}

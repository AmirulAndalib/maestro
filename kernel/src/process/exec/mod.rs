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

//! Program execution routines.
//!
//! Program execution is done in several stages:
//! - Read the program
//! - Parse the program
//! - Build the memory image according to the program
//! - Replace the process's memory with the newly created image to run it

pub mod elf;
pub mod vdso;

use crate::{
	file::{vfs, vfs::ResolutionSettings},
	memory::VirtAddr,
	process::{mem_space::MemSpace, regs::Regs, signal::SignalHandler, Process},
};
use utils::{
	collections::{string::String, vec::Vec},
	errno::EResult,
	lock::{IntMutex, Mutex},
	ptr::arc::Arc,
};

/// Information to prepare a program image to be executed.
pub struct ExecInfo<'s> {
	/// Path resolution settings.
	pub path_resolution: &'s ResolutionSettings,
	/// The list of arguments.
	pub argv: Vec<String>,
	/// The list of environment variables.
	pub envp: Vec<String>,
}

/// A built program image.
pub struct ProgramImage {
	/// The argv of the program.
	argv: Vec<String>,
	/// The environment variables of the program.
	envp: String,

	/// The image's memory space.
	mem_space: MemSpace,

	/// A pointer to the entry point of the program.
	entry_point: VirtAddr,
	/// A pointer to the initial value of the user stack pointer.
	user_stack: VirtAddr,
}

/// A program executor, whose role is to load a program and to prepare it for execution.
pub trait Executor {
	/// Builds a program image.
	/// `file` is the program's file.
	fn build_image(&self, file: &vfs::Entry) -> EResult<ProgramImage>;
}

/// Builds a program image from the given executable file.
///
/// Arguments:
/// - `file` is the program's file
/// - `info` is the set execution information for the program
///
/// The function returns a memory space containing the program image and the
/// pointer to the entry point.
pub fn build_image(file: &vfs::Entry, info: ExecInfo) -> EResult<ProgramImage> {
	// TODO Support other formats than ELF (wasm?)

	let exec = elf::ELFExecutor::new(info)?;
	exec.build_image(file)
}

/// Executes the program image `image` on the process `proc`.
pub fn exec(proc: &mut Process, image: ProgramImage) -> EResult<()> {
	proc.argv = Arc::new(image.argv)?;
	proc.envp = Arc::new(image.envp)?;
	// TODO Set exec path
	// Set the new memory space to the process
	proc.set_mem_space(Some(Arc::new(IntMutex::new(image.mem_space))?));
	// Duplicate the file descriptor table
	proc.file_descriptors = proc
		.file_descriptors
		.as_ref()
		.map(|fds_mutex| -> EResult<_> {
			let fds = fds_mutex.lock();
			let new_fds = fds.duplicate(true)?;
			Ok(Arc::new(Mutex::new(new_fds))?)
		})
		.transpose()?;
	// Reset signals
	proc.signal_handlers.lock().fill(SignalHandler::Default);
	proc.reset_vfork();
	proc.tls_entries = Default::default();
	proc.update_tss();
	// Set the process's registers
	proc.regs = Regs {
		esp: image.user_stack.0,
		eip: image.entry_point.0,
		..Default::default()
	};
	Ok(())
}

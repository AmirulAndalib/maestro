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

//! Implementation of the `self` symlink, which points to the current process's directory.

use crate::{
	file::{fs::NodeOps, FileLocation, FileType, Stat},
	format_content,
	process::Process,
};
use utils::errno::EResult;

/// The `self` symlink.
#[derive(Debug, Default)]
pub struct SelfNode;

impl NodeOps for SelfNode {
	fn get_stat(&self, _loc: &FileLocation) -> EResult<Stat> {
		Ok(Stat {
			file_type: FileType::Link,
			mode: 0o777,
			..Default::default()
		})
	}

	fn read_content(&self, _loc: &FileLocation, off: u64, buf: &mut [u8]) -> EResult<usize> {
		let pid = Process::current().lock().get_pid();
		format_content!(off, buf, "{pid}")
	}
}

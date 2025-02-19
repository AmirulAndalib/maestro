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

//! A filesystem is the representation of the file hierarchy on a storage
//! device.

pub mod ext2;
pub mod initramfs;
pub mod kernfs;
pub mod proc;
pub mod tmp;

use super::{
	perm::{Gid, Uid},
	DirEntry, FileLocation, INode, Mode, Stat,
};
use crate::{device::DeviceIO, sync::mutex::Mutex, time::unit::Timestamp};
use core::{any::Any, ffi::c_int, fmt::Debug};
use utils::{
	boxed::Box,
	collections::{hashmap::HashMap, path::PathBuf, string::String},
	errno,
	errno::{EResult, ENOTDIR},
	ptr::arc::Arc,
};

/// Used in the f_fsid field of [`Statfs`].
///
/// It is currently unused.
#[repr(C)]
#[derive(Debug, Default)]
struct Fsid {
	/// Unused.
	_val: [c_int; 2],
}

/// Statistics about a filesystem.
#[repr(C)]
#[derive(Debug)]
pub struct Statfs {
	/// Type of filesystem.
	f_type: u32,
	/// Optimal transfer block size.
	f_bsize: u32,
	/// Total data blocks in filesystem.
	f_blocks: i64,
	/// Free blocks in filesystem.
	f_bfree: i64,
	/// Free blocks available to unprivileged user.
	f_bavail: i64,
	/// Total inodes in filesystem.
	f_files: i64,
	/// Free inodes in filesystem.
	f_ffree: i64,
	/// Filesystem ID.
	f_fsid: Fsid,
	/// Maximum length of filenames.
	f_namelen: u32,
	/// Fragment size.
	f_frsize: u32,
	/// Mount flags of filesystem.
	f_flags: u32,
}

/// A set of attributes to modify on a file's status.
#[derive(Default)]
pub struct StatSet {
	/// Set the mode of the file.
	pub mode: Option<Mode>,
	/// Set the number of links to the file.
	pub nlink: Option<u16>,
	/// Set the owner's user ID.
	pub uid: Option<Uid>,
	/// Set the owner's group ID.
	pub gid: Option<Gid>,
	/// Set the timestamp of the last modification of the metadata.
	pub ctime: Option<Timestamp>,
	/// Set the timestamp of the last modification of the file's content.
	pub mtime: Option<Timestamp>,
	/// Set the timestamp of the last access to the file.
	pub atime: Option<Timestamp>,
}

/// Filesystem node operations.
pub trait NodeOps: Debug {
	/// Returns the file's status.
	///
	/// `loc` is the location of the file.
	fn get_stat(&self, loc: &FileLocation) -> EResult<Stat>;

	/// Sets the file's status.
	///
	/// Arguments:
	/// - `loc` is the location of the file.
	/// - `set` is the set of status attributes to modify on the file.
	///
	/// The default implementation of this function does nothing.
	fn set_stat(&self, loc: &FileLocation, set: StatSet) -> EResult<()> {
		let _ = (loc, set);
		Ok(())
	}

	/// Reads from the node with into the buffer `buf`.
	///
	/// Arguments:
	/// - `loc` is the location of the file.
	/// - `off` is the offset from which the data will be read from the node's data.
	/// - `buf` is the buffer in which the data is to be written. The length of the buffer is the
	///   number of bytes to read.
	///
	/// This function is relevant for the following file types:
	/// - `Regular`: Reads the content of the file
	/// - `Link`: Reads the path the link points to
	///
	/// The function returns the number of bytes read and whether the *end-of-file* has been
	/// reached.
	///
	/// The default implementation of this function returns an error.
	fn read_content(&self, loc: &FileLocation, off: u64, buf: &mut [u8]) -> EResult<usize> {
		let _ = (loc, off, buf);
		Err(errno!(EINVAL))
	}

	/// Writes to the node from the buffer `buf`.
	///
	/// Arguments:
	/// - `loc` is the location of the file.
	/// - `off` is the offset at which the data will be written in the node's data.
	/// - `buf` is the buffer in which the data is to be read from. The length of the buffer is the
	///   number of bytes to write.
	///
	/// This function is relevant for the following file types:
	/// - `Regular`: Writes the content of the file
	/// - `Link`: Writes the path the link points to. `off` is ignored for links and is always
	///   considered to be zero
	///
	/// The default implementation of this function returns an error.
	fn write_content(&self, loc: &FileLocation, off: u64, buf: &[u8]) -> EResult<usize> {
		let _ = (loc, off, buf);
		Err(errno!(EINVAL))
	}

	/// Changes the size of the file, truncating its content if necessary.
	///
	/// If `size` is greater than or equals to the current size of the file, the function does
	/// nothing.
	///
	/// The default implementation of this function returns an error.
	fn truncate_content(&self, loc: &FileLocation, size: u64) -> EResult<()> {
		let _ = (loc, size);
		Err(errno!(EINVAL))
	}

	/// Returns the directory entry with the given `name`, along with its offset and the handle of
	/// the file.
	///
	/// If the entry does not exist, the function returns `None`.
	///
	/// If the node is not a directory, the function returns [`ENOTDIR`].
	///
	/// The default implementation of this function returns an error.
	fn entry_by_name<'n>(
		&self,
		loc: &FileLocation,
		name: &'n [u8],
	) -> EResult<Option<(DirEntry<'n>, Box<dyn NodeOps>)>> {
		let _ = (loc, name);
		Err(errno!(ENOTDIR))
	}

	/// Returns the directory entry at the given offset `off`. The first entry is always located at
	/// offset `0`.
	///
	/// The second returned value is the offset to the next entry.
	///
	/// If no entry is left, the function returns `None`.
	///
	/// If the node is not a directory, the function returns [`ENOTDIR`].
	///
	/// The default implementation of this function returns an error.
	fn next_entry(
		&self,
		loc: &FileLocation,
		off: u64,
	) -> EResult<Option<(DirEntry<'static>, u64)>> {
		let _ = (loc, off);
		Err(errno!(ENOTDIR))
	}

	/// Helper function to check whether the node is an empty directory.
	///
	/// If the node is not a directory, the function returns `false`.
	fn is_empty_directory(&self, loc: &FileLocation) -> EResult<bool> {
		let mut off = 0;
		loop {
			let res = self.next_entry(loc, off);
			let (ent, next_off) = match res {
				Ok(Some(ent)) => ent,
				Ok(None) => break,
				Err(e) if e.as_int() == ENOTDIR => return Ok(false),
				Err(e) => return Err(e),
			};
			let name = ent.name.as_ref();
			if name != b"." && name != b".." {
				return Ok(false);
			}
			off = next_off;
		}
		Ok(true)
	}

	/// Adds a file into the directory.
	///
	/// Arguments:
	/// - `parent` is the location of the parent directory.
	/// - `name` is the name of the hard link to add.
	/// - `stat` is the status of the file to add.
	///
	/// On success, the function returns the allocated [`INode`] together with the new file's
	/// handle.
	///
	/// The default implementation of this function returns an error.
	fn add_file(
		&self,
		parent: &FileLocation,
		name: &[u8],
		stat: Stat,
	) -> EResult<(INode, Box<dyn NodeOps>)> {
		let _ = (parent, name, stat);
		Err(errno!(ENOTDIR))
	}

	/// Adds a hard link into the directory.
	///
	/// Arguments:
	/// - `parent` is the location of the parent directory.
	/// - `name` is the name of the hard link to add.
	/// - `target` is the inode the link points to.
	///
	/// If this feature is not supported by the filesystem, the function returns
	/// an error.
	///
	/// The default implementation of this function returns an error.
	fn link(&self, parent: &FileLocation, name: &[u8], target: INode) -> EResult<()> {
		let _ = (parent, name, target);
		Err(errno!(ENOTDIR))
	}

	/// Removes a hard link from the directory.
	///
	/// Arguments:
	/// - `parent` is the parent directory.
	/// - `name` is the name of the hard link to remove.
	///
	/// On success, the function returns the number of links to the target node left, along with
	/// the target inode.
	///
	/// If this feature is not supported by the filesystem, the function returns
	/// an error.
	///
	/// The default implementation of this function returns an error.
	fn unlink(&self, parent: &FileLocation, name: &[u8]) -> EResult<()> {
		let _ = (parent, name);
		Err(errno!(ENOTDIR))
	}

	/// Removes a file from the filesystem.
	///
	/// If the file to be removed is a non-empty directory, the function returns
	/// [`errno::ENOTEMPTY`].
	///
	/// The default implementation of this function returns an error.
	fn remove_node(&self, loc: &FileLocation) -> EResult<()> {
		let _ = loc;
		Err(errno!(ENOTDIR))
	}
}

/// A filesystem.
///
/// Type implementing this trait must use of internal mutability to allow multiple threads to
/// perform operations on a filesystem at the same time.
pub trait Filesystem: Any + Debug {
	/// Returns the name of the filesystem.
	fn get_name(&self) -> &[u8];
	/// Tells the kernel can cache the filesystem's files in memory.
	fn use_cache(&self) -> bool;
	/// Returns the root inode of the filesystem.
	fn get_root_inode(&self) -> INode;
	/// Returns statistics about the filesystem.
	fn get_stat(&self) -> EResult<Statfs>;

	/// Returns the node handle for the given `inode`.
	///
	/// If the node does not exist, the function returns [`errno::ENOENT`].
	fn node_from_inode(&self, inode: INode) -> EResult<Box<dyn NodeOps>>;
}

/// Downcasts the given `fs` into `F`.
///
/// If the filesystem type do not match, the function panics.
pub fn downcast_fs<F: Filesystem>(fs: &dyn Filesystem) -> &F {
	(fs as &dyn Any).downcast_ref().unwrap()
}

/// A filesystem type.
pub trait FilesystemType {
	/// Returns the name of the filesystem.
	fn get_name(&self) -> &'static [u8];

	/// Tells whether the given IO interface has the current filesystem.
	///
	/// `io` is the IO interface.
	fn detect(&self, io: &dyn DeviceIO) -> EResult<bool>;

	/// Creates a new instance of the filesystem to mount it.
	///
	/// Arguments:
	/// - `io` is the IO interface.
	/// - `mountpath` is the path on which the filesystem is mounted.
	/// - `readonly` tells whether the filesystem is mounted in read-only.
	fn load_filesystem(
		&self,
		io: Option<Arc<dyn DeviceIO>>,
		mountpath: PathBuf,
		readonly: bool,
	) -> EResult<Arc<dyn Filesystem>>;
}

/// The list of filesystem types.
static FS_TYPES: Mutex<HashMap<String, Arc<dyn FilesystemType>>> = Mutex::new(HashMap::new());

/// Registers a new filesystem type.
pub fn register<T: 'static + FilesystemType>(fs_type: T) -> EResult<()> {
	let name = String::try_from(fs_type.get_name())?;
	let mut fs_types = FS_TYPES.lock();
	fs_types.insert(name, Arc::new(fs_type)?)?;
	Ok(())
}

/// Unregisters the filesystem type with the given name.
///
/// If the filesystem type doesn't exist, the function does nothing.
pub fn unregister(name: &[u8]) {
	let mut fs_types = FS_TYPES.lock();
	fs_types.remove(name);
}

/// Returns the filesystem type with name `name`.
pub fn get_type(name: &[u8]) -> Option<Arc<dyn FilesystemType>> {
	let fs_types = FS_TYPES.lock();
	fs_types.get(name).cloned()
}

/// Detects the filesystem type on the given IO interface `io`.
pub fn detect(io: &dyn DeviceIO) -> EResult<Arc<dyn FilesystemType>> {
	let fs_types = FS_TYPES.lock();
	for (_, fs_type) in fs_types.iter() {
		if fs_type.detect(io)? {
			return Ok(fs_type.clone());
		}
	}
	Err(errno!(ENODEV))
}

/// Registers the filesystems that are implemented inside the kernel itself.
///
/// This function must be called only once, at initialization.
pub fn register_defaults() -> EResult<()> {
	register(ext2::Ext2FsType {})?;
	register(tmp::TmpFsType {})?;
	register(proc::ProcFsType {})?;
	// TODO sysfs
	Ok(())
}

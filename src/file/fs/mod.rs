//! A filesystem is the representation of the file hierarchy on a storage device.

pub mod ext2;
pub mod kernfs;
pub mod tmp;

use crate::errno::Errno;
use crate::errno;
use crate::util::IO;
use crate::util::boxed::Box;
use crate::util::container::string::String;
use crate::util::container::vec::Vec;
use crate::util::lock::Mutex;
use crate::util::ptr::SharedPtr;
use super::File;
use super::INode;
use super::path::Path;

/// Trait representing a filesystem.
pub trait Filesystem {
	/// Returns the name of the filesystem.
	fn get_name(&self) -> &[u8];

	/// Tells whether the filesystem is mounted in read-only.
	fn is_readonly(&self) -> bool;
	/// Tells the kernel whether it must cache files.
	fn must_cache(&self) -> bool;

	/// Returns the inode of the file at path `path`.
	/// `io` is the IO interface.
	/// `path` is the file's path.
	/// The path must be absolute relative the filesystem's root directory and must not contain
	/// any `.` or `..` component.
	fn get_inode(&mut self, io: &mut dyn IO, path: Path) -> Result<INode, Errno>;

	/// Loads the file at inode `inode`.
	/// `io` is the IO interface.
	/// `inode` is the file's inode.
	/// `name` is the file's name.
	fn load_file(&mut self, io: &mut dyn IO, inode: INode, name: String) -> Result<File, Errno>;

	/// Adds a file to the filesystem at inode `inode`.
	/// `io` is the IO interface.
	/// `parent_inode` is the parent file's inode.
	/// `file` is the file to be added.
	/// On success, the function returns the object `file` with the newly created inode set to it.
	fn add_file(&mut self, io: &mut dyn IO, parent_inode: INode, file: File)
		-> Result<File, Errno>;

	/// Removes a file from the filesystem.
	/// `io` is the IO interface.
	/// `parent_inode` is the parent file's inode.
	/// `name` is the file's name.
	fn remove_file(&mut self, io: &mut dyn IO, parent_inode: INode, name: &String)
		-> Result<(), Errno>;

	/// Reads from the given inode `inode` into the buffer `buf`.
	/// `off` is the offset from which the data will be read from the node.
	fn read_node(&mut self, io: &mut dyn IO, inode: INode, off: u64, buf: &mut [u8])
		-> Result<usize, Errno>;

	/// Writes to the given inode `inode` from the buffer `buf`.
	/// `off` is the offset at which the data will be written in the node.
	fn write_node(&mut self, io: &mut dyn IO, inode: INode, off: u64, buf: &[u8])
		-> Result<(), Errno>;
}

/// Trait representing a filesystem type.
pub trait FilesystemType {
	/// Returns the name of the filesystem.
	fn get_name(&self) -> &[u8];

	/// Tells whether the given IO interface has the current filesystem.
	/// `io` is the IO interface.
	fn detect(&self, io: &mut dyn IO) -> Result<bool, Errno>;

	/// Creates a new filesystem on the IO interface and returns its instance.
	/// `io` is the IO interface.
	fn create_filesystem(&self, io: &mut dyn IO) -> Result<Box<dyn Filesystem>, Errno>;

	/// Creates a new instance of the filesystem to mount it.
	/// `io` is the IO interface.
	/// `mountpath` is the path on which the filesystem is mounted.
	/// `readonly` tells whether the filesystem is mounted in read-only.
	fn load_filesystem(&self, io: &mut dyn IO, mountpath: Path, readonly: bool)
		-> Result<Box<dyn Filesystem>, Errno>;
}

/// The list of mountpoints.
static FILESYSTEMS: Mutex<Vec<SharedPtr<dyn FilesystemType>>> = Mutex::new(Vec::new());

/// Registers a new filesystem type `fs`.
pub fn register<T: 'static + FilesystemType>(fs_type: T) -> Result<(), Errno> {
	let mut guard = FILESYSTEMS.lock();
	let container = guard.get_mut();
	container.push(SharedPtr::new(fs_type)?)
}

// TODO Function to unregister a filesystem type

// TODO Optimize
/// Returns the filesystem with name `name`.
pub fn get_fs(name: &[u8]) -> Option<SharedPtr<dyn FilesystemType>> {
	let mut guard = FILESYSTEMS.lock();
	let container = guard.get_mut();

	for i in 0..container.len() {
		let fs_type = &mut container[i];
		let fs_type_guard = fs_type.lock();

		if fs_type_guard.get().get_name() == name {
			drop(fs_type_guard);
			return Some(fs_type.clone());
		}
	}

	None
}

/// Detects the filesystem type on the given IO interface `io`.
pub fn detect(io: &mut dyn IO) -> Result<SharedPtr<dyn FilesystemType>, Errno> {
	let mut guard = FILESYSTEMS.lock();
	let container = guard.get_mut();

	for i in 0..container.len() {
		let fs_type = &mut container[i];
		let fs_type_guard = fs_type.lock();

		if fs_type_guard.get().detect(io)? {
			drop(fs_type_guard);
			return Ok(fs_type.clone()); // TODO Use a weak pointer?
		}
	}

	Err(errno::ENODEV)
}

/// Registers the filesystems that are implemented inside of the kernel itself.
/// This function must be called only once, at initialization.
pub fn register_defaults() -> Result<(), Errno> {
	register(ext2::Ext2FsType {})?;

	Ok(())
}

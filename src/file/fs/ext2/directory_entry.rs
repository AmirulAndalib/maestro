//! A directory entry is an entry stored into an inode's content which represents a subfile in a
//! directory.

use super::Superblock;
use crate::errno::Errno;
use crate::file::FileType;
use crate::memory::malloc;
use crate::util::boxed::Box;
use core::cmp::min;
use core::slice;

/// Directory entry type indicator: Unknown
const TYPE_INDICATOR_UNKNOWN: u8 = 0;
/// Directory entry type indicator: Regular file
const TYPE_INDICATOR_REGULAR: u8 = 1;
/// Directory entry type indicator: Directory
const TYPE_INDICATOR_DIRECTORY: u8 = 2;
/// Directory entry type indicator: Char device
const TYPE_INDICATOR_CHAR_DEVICE: u8 = 3;
/// Directory entry type indicator: Block device
const TYPE_INDICATOR_BLOCK_DEVICE: u8 = 4;
/// Directory entry type indicator: FIFO
const TYPE_INDICATOR_FIFO: u8 = 5;
/// Directory entry type indicator: Socket
const TYPE_INDICATOR_SOCKET: u8 = 6;
/// Directory entry type indicator: Symbolic link
const TYPE_INDICATOR_SYMLINK: u8 = 7;

/// A directory entry is a structure stored in the content of an inode of type Directory. Each
/// directory entry represent a file that is the stored in the directory and points to its inode.
#[repr(C, packed)]
pub struct DirectoryEntry {
	/// The inode associated with the entry.
	inode: u32,
	/// The total size of the entry.
	total_size: u16,
	/// Name length least-significant bits.
	name_length_lo: u8,
	/// Name length most-significant bits or type indicator (if enabled).
	name_length_hi: u8,
	/// The entry's name.
	name: [u8],
}

impl DirectoryEntry {
	/// Creates a new free instance.
	/// `total_size` is the size of the entry, including the name.
	pub fn new_free(total_size: u16) -> Result<Box<Self>, Errno> {
		let slice = unsafe {
			slice::from_raw_parts_mut(malloc::alloc(total_size as _)? as *mut u8, total_size as _)
		};

		let mut entry = unsafe { Box::from_raw(slice as *mut [u8] as *mut [()] as *mut Self) };
		entry.total_size = total_size;
		Ok(entry)
	}

	/// Creates a new instance.
	/// `superblock` is the filesystem's superblock.
	/// `inode` is the entry's inode.
	/// `total_size` is the size of the entry, including the name.
	/// `file_type` is the entry's type.
	/// `name` is the entry's name.
	/// If the total size is not large enough to hold the entry, the behaviour is undefined.
	pub fn new(
		superblock: &Superblock,
		inode: u32,
		total_size: u16,
		file_type: FileType,
		name: &[u8],
	) -> Result<Box<Self>, Errno> {
		debug_assert!(inode >= 1);
		debug_assert!(total_size as usize >= 8 + name.len());

		let mut entry = Self::new_free(total_size)?;
		entry.inode = inode;
		entry.total_size = total_size;
		entry.set_type(superblock, file_type);
		entry.set_name(superblock, name);
		Ok(entry)
	}

	/// Creates a new instance from a slice.
	pub unsafe fn from(slice: &[u8]) -> Result<Box<Self>, Errno> {
		let ptr = malloc::alloc(slice.len())? as *mut u8;
		let alloc_slice = slice::from_raw_parts_mut(ptr, slice.len());
		alloc_slice.copy_from_slice(slice);

		Ok(Box::from_raw(
			alloc_slice as *mut [u8] as *mut [()] as *mut Self,
		))
	}

	/// Returns the entry's inode.
	pub fn get_inode(&self) -> u32 {
		self.inode
	}

	/// Sets the entry's inode.
	/// If `inode` is zero, the entry is set free.
	pub fn set_inode(&mut self, inode: u32) {
		self.inode = inode;
	}

	/// Returns the total size of the entry.
	pub fn get_total_size(&self) -> u16 {
		self.total_size
	}

	/// Returns the length the entry's name.
	/// `superblock` is the filesystem's superblock.
	fn get_name_length(&self, superblock: &Superblock) -> usize {
		if superblock.required_features & super::REQUIRED_FEATURE_DIRECTORY_TYPE == 0 {
			((self.name_length_hi as usize) << 8) | (self.name_length_lo as usize)
		} else {
			self.name_length_lo as usize
		}
	}

	/// Returns the entry's name.
	/// `superblock` is the filesystem's superblock.
	pub fn get_name(&self, superblock: &Superblock) -> &[u8] {
		let name_length = self.get_name_length(superblock);
		&self.name[..name_length]
	}

	/// Sets the name of the entry.
	/// If the length of the entry is shorted than the required space, the name shall be truncated.
	pub fn set_name(&mut self, superblock: &Superblock, name: &[u8]) {
		let len = min(name.len(), self.total_size as usize - 8);
		self.name[..len].copy_from_slice(&name[..len]);

		self.name_length_lo = (len & 0xff) as u8;
		if superblock.required_features & super::REQUIRED_FEATURE_DIRECTORY_TYPE == 0 {
			self.name_length_hi = ((len >> 8) & 0xff) as u8;
		}
	}

	/// Returns the file type associated with the entry (if the option is enabled).
	pub fn get_type(&self, superblock: &Superblock) -> Option<FileType> {
		if superblock.required_features & super::REQUIRED_FEATURE_DIRECTORY_TYPE == 0 {
			match self.name_length_hi {
				TYPE_INDICATOR_REGULAR => Some(FileType::Regular),
				TYPE_INDICATOR_DIRECTORY => Some(FileType::Directory),
				TYPE_INDICATOR_CHAR_DEVICE => Some(FileType::CharDevice),
				TYPE_INDICATOR_BLOCK_DEVICE => Some(FileType::BlockDevice),
				TYPE_INDICATOR_FIFO => Some(FileType::Fifo),
				TYPE_INDICATOR_SOCKET => Some(FileType::Socket),
				TYPE_INDICATOR_SYMLINK => Some(FileType::Link),

				_ => None,
			}
		} else {
			None
		}
	}

	/// Sets the file type associated with the entry (if the option is enabled).
	pub fn set_type(&mut self, superblock: &Superblock, file_type: FileType) {
		if superblock.required_features & super::REQUIRED_FEATURE_DIRECTORY_TYPE != 0 {
			self.name_length_hi = match file_type {
				FileType::Regular => TYPE_INDICATOR_REGULAR,
				FileType::Directory => TYPE_INDICATOR_DIRECTORY,
				FileType::CharDevice => TYPE_INDICATOR_CHAR_DEVICE,
				FileType::BlockDevice => TYPE_INDICATOR_BLOCK_DEVICE,
				FileType::Fifo => TYPE_INDICATOR_FIFO,
				FileType::Socket => TYPE_INDICATOR_SOCKET,
				FileType::Link => TYPE_INDICATOR_SYMLINK,
			};
		}
	}

	/// Tells whether the entry is valid.
	pub fn is_free(&self) -> bool {
		self.inode == 0
	}

	// TODO Preserve alignments of entries at 4 bytes
	/// Splits the current entry into two entries and return the newly created entry.
	/// `new_size` is the size of the current entry.
	/// If the entry is not free, the behaviour is undefined.
	pub fn split(&mut self, new_size: u16) -> Result<Box<Self>, Errno> {
		debug_assert!(self.is_free());

		let new_entry_size = self.total_size - new_size;
		let new_entry = DirectoryEntry::new_free(new_entry_size)?;
		self.total_size = new_size;
		Ok(new_entry)
	}

	/// Merges the current entry with the given entry `entry`.
	/// If both entries are not on the same page or if `entry` is not located right after the
	/// current entry, the behaviour is undefined.
	/// If one the entry is not free, the behaviour is undefined.
	pub fn merge(&mut self, entry: Box<Self>) {
		debug_assert!(self.is_free());
		debug_assert!(entry.is_free());

		self.total_size += entry.total_size;
	}
}

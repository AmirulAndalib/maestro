//! This module implements the `user_desc` structure, which is used in userspace to specify the
//! value for a descriptor, either a local or global descriptor.

use core::ffi::c_void;
use crate::gdt;

/// The size of the user_desc structure in bytes.
const USER_DESC_SIZE: usize = 13;

/// The `user_desc` structure.
#[repr(transparent)]
pub struct UserDesc {
	val: &'static mut [i8; USER_DESC_SIZE],
}

impl UserDesc {
	/// Creates a new instance from the given pointer.
	pub unsafe fn from_ptr(ptr: *mut c_void) -> Self {
		Self {
			val: &mut *(ptr as *mut [i8; USER_DESC_SIZE]),
		}
	}

	/// Returns the entry number.
	#[inline(always)]
	pub fn get_entry_number(&self) -> i32 {
		unsafe { // Safe because the structure is large enough
			*(&self.val[0] as *const _ as *const i32)
		}
	}

	/// Sets the entry number.
	pub fn set_entry_number(&mut self, number: i32) {
		unsafe { // Safe because the structure is large enough
			*(&mut self.val[0] as *mut _ as *mut i32) = number;
		}
	}

	/// Returns the base address.
	#[inline(always)]
	pub fn get_base_addr(&self) -> i32 {
		unsafe { // Safe because the structure is large enough
			*(&self.val[4] as *const _ as *const i32)
		}
	}

	/// Returns the limit.
	#[inline(always)]
	pub fn get_limit(&self) -> i32 {
		unsafe { // Safe because the structure is large enough
			*(&self.val[8] as *const _ as *const i32)
		}
	}

	/// Tells whether the segment is 32 bits.
	#[inline(always)]
	pub fn is_32bits(&self) -> bool {
		(self.val[9] & 0b1) != 0 // TODO Check mask
	}

	/// Tells whether the segment is writable.
	#[inline(always)]
	pub fn is_writable(&self) -> bool {
		(self.val[9] & 0b1000) == 0 // TODO Check mask
	}

	/// Tells whether the segment's limit is in number of pages.
	#[inline(always)]
	pub fn is_limit_in_pages(&self) -> bool {
		(self.val[9] & 0b10000) != 0 // TODO Check mask
	}

	/// Tells whether the segment is present.
	#[inline(always)]
	pub fn is_present(&self) -> bool {
		(self.val[9] & 0b100000) == 0 // TODO Check mask
	}

	/// Tells whether the segment is usable.
	#[inline(always)]
	pub fn is_usable(&self) -> bool {
		(self.val[9] & 0b1000000) != 0 // TODO Check mask
	}

	/// Converts the current descriptor to a GDT entry.
	pub fn to_descriptor(&self) -> gdt::Entry {
        let mut entry = gdt::Entry::default();

	entry.set_base(self.get_base_addr() as _);
        entry.set_limit(self.get_limit() as _);

        let mut access_byte = 0b01100010;
        if self.is_present() && self.is_usable() {
            access_byte |= 1 << 7;
        }
        entry.set_access_byte(access_byte);

        let mut flags = 0b0000;
        if self.is_32bits() {
            flags |= 1 << 2;
        }
        if self.is_limit_in_pages() {
            flags |= 1 << 3;
        }
        entry.set_flags(flags);

        entry
	}
}

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

//! Userspace memory access check wrappers.
//!
//! When a pointer is passed to the kernel through a system call, the kernel is
//! required to check the process is allowed to access it to ensure safety.
//!
//! Structures in this module are especially useful in cases where several processes
//! share the same memory space, making it possible to revoke the access to the
//! pointer while it is being used.
//!
//! Those structures are also usable as system call arguments.

use super::MemSpace;
use crate::process::Process;
use core::{
	fmt,
	mem::size_of,
	ptr::{null, null_mut, NonNull},
	slice,
};
use utils::{errno, errno::EResult, DisplayableStr};

/// Wrapper for a pointer.
pub struct SyscallPtr<T: Sized>(Option<NonNull<T>>);

impl<T: Sized> From<usize> for SyscallPtr<T> {
	/// Creates an instance from a register value.
	fn from(val: usize) -> Self {
		Self(NonNull::new(val as _))
	}
}

impl<T: Sized> SyscallPtr<T> {
	/// Tells whether the pointer is null.
	pub fn is_null(&self) -> bool {
		self.0.is_none()
	}

	/// Returns an immutable pointer to the data.
	pub fn as_ptr(&self) -> *const T {
		self.0.as_ref().map(|p| p.as_ptr() as _).unwrap_or(null())
	}

	/// Returns a mutable pointer to the data.
	pub fn as_ptr_mut(&self) -> *mut T {
		self.0
			.as_ref()
			.map(|p| p.as_ptr() as _)
			.unwrap_or(null_mut())
	}

	/// Returns an immutable reference to the value of the pointer.
	///
	/// If the pointer is null, the function returns `None`.
	///
	/// If the value is not accessible, the function returns an error.
	pub fn get<'a>(&self, mem_space: &'a MemSpace) -> EResult<Option<&'a T>> {
		let Some(ptr) = self.0 else {
			return Ok(None);
		};
		if !mem_space.can_access(ptr.as_ptr() as _, size_of::<T>(), true, false) {
			return Err(errno!(EFAULT));
		}
		// Safe because access is checked before
		Ok(Some(unsafe { ptr.as_ref() }))
	}

	/// Returns a mutable reference to the value of the pointer.
	///
	/// If the pointer is null, the function returns `None`.
	///
	/// If the value is not accessible, the function returns an error.
	///
	/// If the value is located on lazily allocated pages, the function
	/// allocates physical pages in order to allow writing.
	pub fn get_mut<'a>(&self, mem_space: &'a mut MemSpace) -> EResult<Option<&'a mut T>> {
		let Some(mut ptr) = self.0 else {
			return Ok(None);
		};
		if !mem_space.can_access(ptr.as_ptr() as _, size_of::<T>(), true, true) {
			return Err(errno!(EFAULT));
		}
		// Allocate memory to make sure it is writable
		mem_space.alloc(ptr.as_ptr() as _, size_of::<T>())?;
		// Safe because access is checked before
		Ok(Some(unsafe { ptr.as_mut() }))
	}
}

impl<T: fmt::Debug> fmt::Debug for SyscallPtr<T> {
	fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
		let proc_mutex = Process::current_assert();
		let proc = proc_mutex.lock();
		let mem_space_mutex = proc.get_mem_space().unwrap();
		let mem_space = mem_space_mutex.lock();
		let ptr = self.as_ptr();
		match self.get(&mem_space) {
			Ok(Some(val)) => write!(fmt, "{ptr:p} = {val:?}"),
			Ok(None) => write!(fmt, "NULL"),
			Err(e) => write!(fmt, "{ptr:p} = (cannot read: {e})"),
		}
	}
}

/// Wrapper for a slice.
///
/// The size of the slice is required when trying to access it.
pub struct SyscallSlice<T: Sized>(Option<NonNull<T>>);

impl<T: Sized> From<usize> for SyscallSlice<T> {
	/// Creates an instance from a register value.
	fn from(val: usize) -> Self {
		Self(NonNull::new(val as _))
	}
}

impl<T: Sized> SyscallSlice<T> {
	/// Tells whether the pointer is null.
	pub fn is_null(&self) -> bool {
		self.0.is_none()
	}

	/// Returns an immutable pointer to the data.
	pub fn as_ptr(&self) -> *const T {
		self.0.as_ref().map(|p| p.as_ptr() as _).unwrap_or(null())
	}

	/// Returns a mutable pointer to the data.
	pub fn as_ptr_mut(&self) -> *mut T {
		self.0
			.as_ref()
			.map(|p| p.as_ptr() as _)
			.unwrap_or(null_mut())
	}

	/// Returns an immutable reference to the slice.
	///
	/// `len` is the in number of elements in the slice.
	///
	/// If the slice is not accessible, the function returns an error.
	pub fn get<'a>(&self, mem_space: &'a MemSpace, len: usize) -> EResult<Option<&'a [T]>> {
		let Some(ptr) = self.0 else {
			return Ok(None);
		};
		let size = size_of::<T>() * len;
		if !mem_space.can_access(ptr.as_ptr() as _, size, true, false) {
			return Err(errno!(EFAULT));
		}
		Ok(Some(unsafe {
			// Safe because access is checked before
			slice::from_raw_parts(ptr.as_ptr(), len)
		}))
	}

	/// Returns a mutable reference to the slice.
	///
	/// `len` is the in number of elements in the slice.
	///
	/// If the slice is not accessible, the function returns an error.
	///
	/// If the slice is located on lazily allocated pages, the function
	/// allocates physical pages in order to allow writing.
	pub fn get_mut<'a>(
		&self,
		mem_space: &'a mut MemSpace,
		len: usize,
	) -> EResult<Option<&'a mut [T]>> {
		let Some(ptr) = self.0 else {
			return Ok(None);
		};
		let size = size_of::<T>() * len;
		if !mem_space.can_access(ptr.as_ptr() as _, size, true, true) {
			return Err(errno!(EFAULT));
		}
		// Allocate memory to make sure it is writable
		mem_space.alloc(ptr.as_ptr() as _, size)?;
		Ok(Some(unsafe {
			// Safe because access is checked before
			slice::from_raw_parts_mut(ptr.as_ptr(), len)
		}))
	}
}

impl<T: fmt::Debug> fmt::Debug for SyscallSlice<T> {
	fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
		// TODO Print value? (how to get the length of the slice?)
		let ptr = self.as_ptr();
		if !ptr.is_null() {
			write!(fmt, "{ptr:p}")
		} else {
			write!(fmt, "NULL")
		}
	}
}

/// Wrapper for a C-style, nul-terminated (`\0`) string.
pub struct SyscallString(Option<NonNull<u8>>);

impl From<usize> for SyscallString {
	/// Creates an instance from a register value.
	fn from(val: usize) -> Self {
		Self(NonNull::new(val as _))
	}
}

impl SyscallString {
	/// Tells whether the pointer is null.
	pub fn is_null(&self) -> bool {
		self.0.is_none()
	}

	/// Returns an immutable pointer to the data.
	pub fn as_ptr(&self) -> *const u8 {
		self.0.as_ref().map(|p| p.as_ptr() as _).unwrap_or(null())
	}

	/// Returns an immutable reference to the string.
	///
	/// If the string is not accessible, the function returns an error.
	pub fn get<'a>(&self, mem_space: &'a MemSpace) -> EResult<Option<&'a [u8]>> {
		let Some(ptr) = self.0 else {
			return Ok(None);
		};
		let len = mem_space
			.can_access_string(ptr.as_ptr(), true, false)
			.ok_or_else(|| errno!(EFAULT))?;
		Ok(Some(unsafe {
			// Safe because access is checked before
			slice::from_raw_parts(ptr.as_ptr(), len)
		}))
	}
}

impl fmt::Debug for SyscallString {
	fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
		let proc_mutex = Process::current_assert();
		let proc = proc_mutex.lock();
		let mem_space_mutex = proc.get_mem_space().unwrap();
		let mem_space = mem_space_mutex.lock();
		let ptr = self.as_ptr();
		match self.get(&mem_space) {
			Ok(Some(s)) => {
				// TODO Add backslashes to escape `"` and `\`
				let s = DisplayableStr(s);
				write!(fmt, "{ptr:p} = \"{s}\"")
			}
			Ok(None) => write!(fmt, "NULL"),
			Err(e) => write!(fmt, "{ptr:p} = (cannot read: {e})"),
		}
	}
}

/// Wrapper for a C-style, NULL-terminated string array.
pub struct SyscallArray(Option<NonNull<*const u8>>);

impl From<usize> for SyscallArray {
	/// Creates an instance from a register value.
	fn from(val: usize) -> Self {
		Self(NonNull::new(val as _))
	}
}

impl SyscallArray {
	/// Tells whether the pointer is null.
	pub fn is_null(&self) -> bool {
		self.0.is_none()
	}

	/// Returns an immutable pointer to the data.
	pub fn as_ptr(&self) -> *const u8 {
		self.0.as_ref().map(|p| p.as_ptr() as _).unwrap_or(null())
	}

	/// Returns an iterator over the array's elements.
	pub fn iter<'a>(
		&'a self,
		mem_space: &'a MemSpace,
	) -> impl Iterator<Item = EResult<&'a [u8]>> + 'a {
		SyscallArrayIterator {
			mem_space,
			arr: self,
			i: 0,
		}
	}
}

impl fmt::Debug for SyscallArray {
	fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
		let proc_mutex = Process::current_assert();
		let proc = proc_mutex.lock();
		let mem_space_mutex = proc.get_mem_space().unwrap();
		let mem_space = mem_space_mutex.lock();
		let mut list = fmt.debug_list();
		let mut list_ref = &mut list;
		for elem in self.iter(&mem_space) {
			list_ref = match elem {
				Ok(s) => list_ref.entry(&DisplayableStr(s)),
				Err(e) => list_ref.entry(&e),
			};
		}
		list_ref.finish()
	}
}

pub struct SyscallArrayIterator<'a> {
	/// The memory space.
	mem_space: &'a MemSpace,
	/// The array.
	arr: &'a SyscallArray,
	/// The current index.
	i: usize,
}

impl<'a> Iterator for SyscallArrayIterator<'a> {
	type Item = EResult<&'a [u8]>;

	fn next(&mut self) -> Option<Self::Item> {
		let Some(arr) = self.arr.0 else {
			return Some(Err(errno!(EFAULT)));
		};
		// If reaching the end of the array, stop
		let str_ptr = unsafe { arr.add(self.i).read_volatile() };
		if str_ptr.is_null() {
			return None;
		}
		// Get string
		let string: SyscallString = (str_ptr as usize).into();
		let string = string
			.get(self.mem_space)
			.and_then(|s| s.ok_or_else(|| errno!(EFAULT)));
		self.i += 1;
		Some(string)
	}
}

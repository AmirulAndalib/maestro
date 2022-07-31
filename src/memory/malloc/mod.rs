//! This module implements the memory allocation utility for kernelside operations.
//! An unsafe interface is provided, inspired from the C language's malloc interface.
//! The module also provides the structure `Alloc` which safely manages a memory allocation.

mod block;
mod chunk;

use crate::errno;
use crate::errno::Errno;
use crate::memory;
use crate::memory::malloc::ptr::NonNull;
use crate::util;
use crate::util::list::ListNode;
use crate::util::lock::IntMutex;
use block::Block;
use chunk::Chunk;
use core::cmp::min;
use core::cmp::Ordering;
use core::ffi::c_void;
use core::mem::size_of;
use core::ops::Index;
use core::ops::IndexMut;
use core::ptr;
use core::ptr::drop_in_place;
use core::slice;

/// The allocator's mutex.
static MUTEX: IntMutex<()> = IntMutex::new(());

/// Initializes the memory allocator.
pub fn init() {
	chunk::init_free_lists();
}

/// Allocates `n` bytes of kernel memory and returns a pointer to the beginning of the allocated
/// chunk. If the allocation fails, the function shall return an error.
///
/// # Safety
///
/// Allocated pointer must always be freed. Failure to do so results in a memory leak.
/// Writing outside of the allocated range (buffer overflow) results in an undefined behaviour.
pub unsafe fn alloc(n: usize) -> Result<*mut c_void, Errno> {
	let _ = MUTEX.lock();

	if n == 0 {
		return Err(errno!(EINVAL));
	}

	let chunk = chunk::get_available_chunk(n)?.get_chunk();
	chunk.split(n);

	#[cfg(config_debug_malloc_check)]
	chunk.check();
	debug_assert!(chunk.get_size() >= n);
	assert!(!chunk.is_used());
	chunk.set_used(true);

	let ptr = chunk.get_ptr();
	debug_assert!(util::is_aligned(ptr, chunk::ALIGNEMENT));
	debug_assert!(ptr as usize >= memory::PROCESS_END as usize);
	util::bzero(ptr, n);
	Ok(ptr)
}

/// Changes the size of the memory previously allocated with `alloc`. `ptr` is the pointer to the
/// chunk of memory.
/// `n` is the new size of the chunk of memory.
/// If the reallocation fails, the chunk is left untouched and the function returns an error.
pub unsafe fn realloc(ptr: *mut c_void, n: usize) -> Result<*mut c_void, Errno> {
	let _ = MUTEX.lock();

	if n == 0 {
		return Err(errno!(EINVAL));
	}

	let chunk = Chunk::from_ptr(ptr);
	#[cfg(config_debug_malloc_check)]
	chunk.check();
	assert!(chunk.is_used());

	let chunk_size = chunk.get_size();
	match n.cmp(&chunk_size) {
		Ordering::Less => {
			chunk.shrink(chunk_size - n);
			Ok(ptr)
		}

		Ordering::Greater => {
			if !chunk.grow(n - chunk_size) {
				let new_ptr = alloc(n)?;
				util::memcpy(new_ptr, ptr, min(chunk.get_size(), n));
				free(ptr);
				Ok(new_ptr)
			} else {
				Ok(ptr)
			}
		}

		Ordering::Equal => Ok(ptr),
	}
}

/// Frees the memory at the pointer `ptr` previously allocated with `alloc`.
///
/// # Safety
///
/// If `ptr` doesn't point to a valid chunk of memory allocated with the `alloc` function, the
/// behaviour is undefined.
/// Using memory after it was freed causes an undefined behaviour.
pub unsafe fn free(ptr: *mut c_void) {
	let _ = MUTEX.lock();

	let chunk = Chunk::from_ptr(ptr);
	#[cfg(config_debug_malloc_check)]
	chunk.check();
	assert!(chunk.is_used());

	chunk.set_used(false);
	ptr::write_volatile(&mut chunk.as_free_chunk().free_list, ListNode::new_single());

	let c = chunk.coalesce();
	if c.list.is_single() {
		drop_in_place(Block::from_first_chunk(c));
	} else {
		c.as_free_chunk().free_list_insert();
	}
}

/// Structure representing a kernelside allocation.
/// The structure holds one or more elements of the given type. Freeing the allocation doesn't call
/// `drop` on its elements.
#[derive(Debug)]
pub struct Alloc<T> {
	/// Slice representing the allocation.
	slice: NonNull<[T]>,
}

impl<T> Alloc<T> {
	/// Allocates `size` element in the kernel memory and returns a structure wrapping a slice
	/// allowing to access it. If the allocation fails, the function shall return an error.
	///
	/// # Safety
	///
	/// To use this function, one must ensure that zero memory is not an inconsistent state for the
	/// object `T`.
	pub unsafe fn new_zero(size: usize) -> Result<Self, Errno> {
		let slice = NonNull::new({
			let ptr = alloc(size * size_of::<T>())?;
			slice::from_raw_parts_mut::<T>(ptr as _, size)
		})
		.unwrap();

		Ok(Self { slice })
	}

	/// Returns an immutable reference to the underlying slice.
	pub fn as_slice(&self) -> &[T] {
		unsafe { &*self.slice.as_ref() }
	}

	/// Returns a mutable reference to the underlying slice.
	pub fn as_slice_mut(&mut self) -> &mut [T] {
		unsafe { self.slice.as_mut() }
	}

	/// Returns the allocation as pointer.
	pub unsafe fn as_ptr(&self) -> *const T {
		self.as_slice().as_ptr() as _
	}

	/// Returns the allocation as mutable pointer.
	pub unsafe fn as_ptr_mut(&mut self) -> *mut T {
		self.as_slice_mut().as_mut_ptr() as _
	}

	/// Returns the size of the allocation in number of elements.
	pub fn len(&self) -> usize {
		self.slice.len()
	}

	/// Changes the size of the memory allocation. All new elements are initialized to zero.
	/// `n` is the new size of the chunk of memory (in number of elements).
	/// If the reallocation fails, the chunk is left untouched and the function returns an error.
	///
	/// # Safety
	///
	/// To use this function, one must ensure that zero memory is not an inconsistent state for the
	/// object `T`.
	pub unsafe fn realloc_zero(&mut self, n: usize) -> Result<(), Errno> {
		let ptr = realloc(self.as_ptr_mut() as _, n * size_of::<T>())?;
		self.slice = NonNull::new(slice::from_raw_parts_mut::<T>(ptr as _, n)).unwrap();

		Ok(())
	}

	/// Frees the allocation.
	pub fn free(self) {}
}

impl<T: Default> Alloc<T> {
	/// Allocates `size` element in the kernel memory and returns a structure wrapping a slice
	/// allowing to access it. If the allocation fails, the function shall return an error.
	/// The function will fill the memory with the default value for the object T.
	pub fn new_default(size: usize) -> Result<Self, Errno> {
		let mut alloc = unsafe {
			// Safe because the memory is set right after
			Self::new_zero(size)?
		};
		for i in alloc.as_slice_mut().iter_mut() {
			*i = T::default();
		}

		Ok(alloc)
	}

	/// Resizes the current allocation. If new elements are added, the function initializes them
	/// with the default value.
	/// `n` is the new size of the chunk of memory (in number of elements).
	///
	/// # Safety
	///
	/// If elements are removed, the function `drop` is **not** called on them. Thus, the caller
	/// must take care of dropping the elements first.
	pub unsafe fn realloc_default(&mut self, n: usize) -> Result<(), Errno> {
		let curr_size = self.len();
		self.realloc_zero(n)?;

		for i in curr_size..n {
			self.as_slice_mut()[i] = T::default();
		}

		Ok(())
	}
}

impl<T: Clone> Alloc<T> {
	/// Allocates `size` element in the kernel memory and returns a structure wrapping a slice
	/// allowing to access it. If the allocation fails, the function shall return an error.
	/// `val` is a value that will be cloned to fill the memory.
	pub fn new_clonable(size: usize, val: T) -> Result<Self, Errno> {
		let mut alloc = unsafe {
			// Safe because the memory is set right after
			Self::new_zero(size)?
		};
		for i in alloc.as_slice_mut().iter_mut() {
			*i = val.clone();
		}

		Ok(alloc)
	}
}

impl<T> Index<usize> for Alloc<T> {
	type Output = T;

	#[inline]
	fn index(&self, index: usize) -> &Self::Output {
		&self.as_slice()[index]
	}
}

impl<T> IndexMut<usize> for Alloc<T> {
	#[inline]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		&mut self.as_slice_mut()[index]
	}
}

impl<T> Drop for Alloc<T> {
	fn drop(&mut self) {
		unsafe {
			free(self.as_ptr_mut() as _);
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::memory;
	use crate::util::math;

	#[test_case]
	fn alloc_free0() {
		unsafe {
			assert!(alloc(0).is_err());
		}
	}

	#[test_case]
	fn alloc_free1() {
		unsafe {
			let ptr = alloc(1).unwrap();
			util::memset(ptr, -1, 1);
			free(ptr);
		}
	}

	#[test_case]
	fn alloc_free1() {
		unsafe {
			let ptr = alloc(8).unwrap();
			util::memset(ptr, -1, 8);
			free(ptr);
		}
	}

	#[test_case]
	fn alloc_free2() {
		unsafe {
			let ptr = alloc(memory::PAGE_SIZE).unwrap();
			util::memset(ptr, -1, memory::PAGE_SIZE);
			free(ptr);
		}
	}

	#[test_case]
	fn alloc_free3() {
		unsafe {
			let ptr = alloc(memory::PAGE_SIZE * 10).unwrap();
			util::memset(ptr, -1, memory::PAGE_SIZE * 10);
			free(ptr);
		}
	}

	#[test_case]
	fn alloc_free_fifo() {
		unsafe {
			let mut ptrs: [*mut c_void; 1024] = [0 as _; 1024];

			for i in 0..ptrs.len() {
				let size = i + 1;
				let ptr = alloc(size).unwrap();
				util::memset(ptr, -1, size);
				ptrs[i] = ptr;
			}

			for i in 0..ptrs.len() {
				for j in (i + 1)..ptrs.len() {
					assert!(ptrs[j] != ptrs[i]);
				}
			}

			for p in ptrs.iter() {
				free(*p);
			}
		}
	}

	fn lifo_test(i: usize) {
		unsafe {
			let ptr = alloc(i).unwrap();
			util::memset(ptr, -1, i);
			if i > 1 {
				lifo_test(i - 1);
			}
			free(ptr);
		}
	}

	#[test_case]
	fn alloc_free_lifo() {
		lifo_test(100);
	}

	// TODO Check the integrity of the data after reallocation
	#[test_case]
	fn realloc0() {
		unsafe {
			let mut ptr = alloc(1).unwrap();

			for i in 1..memory::PAGE_SIZE {
				ptr = realloc(ptr, i).unwrap();
				util::memset(ptr, -1, i);
			}

			free(ptr);
		}
	}

	// TODO Check the integrity of the data after reallocation
	#[test_case]
	fn realloc1() {
		unsafe {
			let mut ptr = alloc(memory::PAGE_SIZE).unwrap();

			for i in (1..memory::PAGE_SIZE).rev() {
				ptr = realloc(ptr, i).unwrap();
				util::memset(ptr, -1, i);
			}

			free(ptr);
		}
	}

	// TODO Check the integrity of the data after reallocation
	#[test_case]
	fn realloc2() {
		unsafe {
			let mut ptr0 = alloc(8).unwrap();
			let mut ptr1 = alloc(8).unwrap();
			util::memset(ptr0, -1, 8);
			util::memset(ptr1, -1, 8);

			for i in 0..8 {
				ptr0 = realloc(ptr0, math::pow2(i)).unwrap();
				ptr1 = realloc(ptr1, math::pow2(i) + 1).unwrap();
			}

			free(ptr1);
			free(ptr0);
		}
	}

	// TODO Check the integrity of the data after reallocation
	#[test_case]
	fn realloc3() {
		unsafe {
			let mut ptr0 = alloc(8).unwrap();
			let mut ptr1 = alloc(8).unwrap();
			util::memset(ptr0, -1, 8);
			util::memset(ptr1, -1, 8);

			for i in (0..8).rev() {
				ptr0 = realloc(ptr0, math::pow2(i)).unwrap();
				ptr1 = realloc(ptr1, math::pow2(i) + 1).unwrap();
			}

			free(ptr1);
			free(ptr0);
		}
	}
}

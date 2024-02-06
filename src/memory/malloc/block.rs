//! In the malloc allocator, a block is a memory allocation performed from
//! another allocator, which is too big to be used directly for allocation, so
//! it has to be divided into chunks.

use super::chunk::{Chunk, FreeChunk};
use crate::{errno::AllocResult, memory, memory::buddy};
use core::{
	mem::{offset_of, size_of},
	num::NonZeroUsize,
	ptr,
};

/// A frame of memory allocated using the buddy allocator, storing memory chunks.
#[repr(C, align(8))]
pub struct Block {
	/// The order of the frame for the buddy allocator
	order: buddy::FrameOrder,
	/// The first chunk of the block
	pub first_chunk: Chunk,
}

impl Block {
	/// Allocates a new block of memory with the minimum available size
	/// `min_size` in bytes.
	///
	/// The buddy allocator must be initialized before using this function.
	///
	/// The underlying chunk created by this function is **not** inserted into the free list.
	pub fn new(min_size: NonZeroUsize) -> AllocResult<&'static mut Self> {
		let min_total_size = size_of::<Block>() + min_size.get();
		let block_order = buddy::get_order(min_total_size.div_ceil(memory::PAGE_SIZE));
		// The size of the first chunk
		let first_chunk_size = buddy::get_frame_size(block_order) - size_of::<Block>();
		debug_assert!(first_chunk_size >= min_size.get());
		// Allocate the block
		let block = unsafe {
			let mut ptr = buddy::alloc_kernel(block_order)?.cast();
			ptr::write_volatile(
				ptr.as_mut(),
				Self {
					order: block_order,
					first_chunk: Chunk::new(),
				},
			);
			ptr.as_mut()
		};
		*block.first_chunk.as_free_chunk().unwrap() = FreeChunk::new(first_chunk_size);
		Ok(block)
	}

	/// Returns a mutable reference to the block whose first chunk's reference
	/// is passed as argument.
	pub unsafe fn from_first_chunk(chunk: *mut Chunk) -> &'static mut Block {
		let first_chunk_off = offset_of!(Block, first_chunk);
		let ptr = ((chunk as usize) - first_chunk_off) as *mut Self;
		debug_assert!(ptr.is_aligned_to(memory::PAGE_SIZE));
		&mut *ptr
	}
}

impl Drop for Block {
	fn drop(&mut self) {
		unsafe {
			buddy::free_kernel(self as *mut _ as _, self.order);
		}
	}
}

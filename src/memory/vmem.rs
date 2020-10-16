/*
 * This file must be compiled for x86 only.
 * The virtual memory makes the kernel able to isolate processes, which is essential for modern
 * systems.
 *
 * x86 virtual memory works with a tree structure. Each element is an array of subelements. The
 * position of the elements in the arrays allows to tell the virtual address for the mapping. Under
 * 32 bits, elements are array of 32 bits long words that can contain 1024 entries. The following
 * elements are available:
 * - Page directory: The main element, contains page tables
 * - Page table: Represents a block of 4MB, each entry is a page
 *
 * Under 32 bits, pages are 4096 bytes large. Each entries of elements contains the physical
 * address to the element/page and some flags. The flags can be stored with the address in only
 * 4 bytes large entries because addresses have to be page-aligned, freeing 12 bits in the entry
 * for the flags.
 *
 * For each entries of each elements, the kernel must keep track of how many elements are being
 * used. This can be done with a simple counter: when an entry is allocated, the counter is
 * incremented and when an entry is freed, the counter is decremented. When the counter reaches 0,
 * the element can be freed.
 *
 * The Page Size Extension (PSE) allows to map 4MB large blocks using only a page directory entry.
 */

use core::result::Result;
use crate::elf;
use crate::memory::NULL;
use crate::memory::Void;
use crate::memory::buddy;
use crate::memory;
use crate::multiboot;
use crate::util;

/*
 * x86 paging flag. If set, pages are 4 MB long.
 */
pub const PAGING_TABLE_PAGE_SIZE: u32 = 0b10000000;
/*
 * x86 paging flag. Set if the page has been read or wrote.
 */
pub const PAGING_TABLE_ACCESSED: u32 = 0b00100000;
/*
 * x86 paging flag. If set, page will not be cached.
 */
pub const PAGING_TABLE_CACHE_DISABLE: u32 = 0b00010000;
/*
 * x86 paging flag. If set, write-through caching is enabled.
 * If not, then write-back is enabled instead.
 */
pub const PAGING_TABLE_WRITE_THROUGH: u32 = 0b00001000;
/*
 * x86 paging flag. If set, the page can be accessed by userspace operations.
 */
pub const PAGING_TABLE_USER: u32 = 0b00000100;
/*
 * x86 paging flag. If set, the page can be wrote.
 */
pub const PAGING_TABLE_WRITE: u32 = 0b00000010;
/*
 * x86 paging flag. If set, the page is present.
 */
pub const PAGING_TABLE_PRESENT: u32 = 0b00000001;

/*
 * TODO doc
 */
pub const PAGING_PAGE_GLOBAL: u32 = 0b100000000;
/*
 * TODO doc
 */
pub const PAGING_PAGE_DIRTY: u32 = 0b001000000;
/*
 * TODO doc
 */
pub const PAGING_PAGE_ACCESSED: u32 = 0b000100000;
/*
 * TODO doc
 */
pub const PAGING_PAGE_CACHE_DISABLE: u32 = 0b000010000;
/*
 * TODO doc
 */
pub const PAGING_PAGE_WRITE_THROUGH: u32 = 0b000001000;
/*
 * TODO doc
 */
pub const PAGING_PAGE_USER: u32 = 0b000000100;
/*
 * TODO doc
 */
pub const PAGING_PAGE_WRITE: u32 = 0b000000010;
/*
 * TODO doc
 */
pub const PAGING_PAGE_PRESENT: u32 = 0b000000001;

/*
 * Flags mask in a page directory entry.
 */
pub const PAGING_FLAGS_MASK: u32 = 0xfff;
/*
 * Address mask in a page directory entry. The address doesn't need every bytes
 * since it must be page-aligned.
 */
pub const PAGING_ADDR_MASK: u32 = !PAGING_FLAGS_MASK;

/*
 * x86 page fault flag. If set, the page was present.
 */
pub const PAGE_FAULT_PRESENT: u32 = 0b00001;
/*
 * x86 page fault flag. If set, the error was caused bt a write operation, else
 * the error was caused by a read operation.
 */
pub const PAGE_FAULT_WRITE: u32 = 0b00010;
/*
 * x86 page fault flag. If set, the page fault was caused by a userspace
 * operation.
 */
pub const PAGE_FAULT_USER: u32 = 0b00100;
/*
 * x86 page fault flag. If set, one or more page directory entries contain
 * reserved bits which are set.
 */
pub const PAGE_FAULT_RESERVED: u32 = 0b01000;
/*
 * x86 page fault flag. If set, the page fault was caused by an instruction
 * fetch.
 */
pub const PAGE_FAULT_INSTRUCTION: u32 = 0b10000;

/*
 * The type representing a x86 page directory.
 */
type VMem = *const u32;
/*
 * Same as VMem, but mutable.
 */
type MutVMem = *mut u32;

extern "C" {
	pub fn cr0_get() -> u32;
	pub fn cr0_set(flags: u32);
	pub fn cr0_clear(flags: u32);
	pub fn cr2_get() -> u32;
	pub fn cr3_get() -> u32;

	fn paging_enable(directory: *const u32);
	fn paging_disable();
	fn tlb_reload();
}

/*
 * Structure wrapping a virtual memory. This structure contains the counter for the number of
 * elements that are used in the associated element.
 */
pub struct VMemWrapper {
	/* The number of used elements in the associated element */
	used_elements: u16,
	/* The associated element */
	vmem: VMem,
}

// TODO Find a place to store wrappers

/*
 * Protects the kernel's read-only sections from writing in the given page directory `vmem`.
 */
fn protect_kernel(vmem: MutVMem) {
	let boot_info = multiboot::get_boot_info();
	elf::foreach_sections(boot_info.elf_sections, boot_info.elf_num as usize,
		boot_info.elf_shndx as usize, boot_info.elf_entsize as usize,
		| section: &elf::ELF32SectionHeader, _name: &str | {
			if section.sh_flags & elf::SHF_WRITE != 0
				|| section.sh_addralign as usize != memory::PAGE_SIZE {
				return;
			}

			let pages = util::ceil_division(section.sh_size, memory::PAGE_SIZE as _) as usize;
			map_range(vmem, (memory::PROCESS_END as usize + section.sh_addr as usize) as _,
				section.sh_addr as *const Void, pages as usize, PAGING_PAGE_USER);
		});
}

/*
 * Initializes a new page directory. The kernel memory is mapped into the context by default.
 */
pub fn init() -> Result<MutVMem, ()> {
	let v = buddy::alloc_zero(0, buddy::FLAG_ZONE_TYPE_KERNEL)? as MutVMem;
	// TODO If Meltdown mitigation is enabled, only allow read access to a stub for interrupts
	map_range(v, NULL, memory::PROCESS_END, 262144, PAGING_PAGE_WRITE); // TODO Place pages count in a constant
	protect_kernel(v);
	Ok(v)
}

/*
 * Creates and loads the kernel's page directory. The kernel's code is protected from writing.
 */
pub fn kernel() {
	if let Ok(kernel_vmem) = init() {
		unsafe {
			paging_enable(kernel_vmem);
		}
	} else {
		::kernel_panic!("Cannot initialize kernel virtual memory!", 0);
	}
}

/*
 * Returns the index of the element corresponding to the given virtual address `ptr` for element at
 * level `level` in the tree. The level represents the depth in the tree. `0` is the deepest.
 */
fn get_addr_element_index(ptr: *const Void, level: usize) -> usize {
	((ptr as usize) >> (12 + level * 10)) & 0x3ff
}

// TODO Adapt to 5 level paging
/*
 * Resolves the paging entry for the given pointer. If no entry is found, None is returned. The
 * entry must be marked as present to be found. If Page Size Extension (PSE) is used, an entry of
 * the page directory might be returned.
 */
pub fn resolve(vmem: VMem, ptr: *const Void) -> Option<*const u32> {
	let dir_entry = unsafe { vmem.add(get_addr_element_index(ptr, 1)) };
	let dir_entry_value = unsafe { *dir_entry };
	if dir_entry_value & PAGING_TABLE_PRESENT == 0 {
		return None;
	}
	if dir_entry_value & PAGING_TABLE_PAGE_SIZE == 0 {
		return Some(dir_entry);
	}

	let dir_entry_ptr = (dir_entry_value & PAGING_ADDR_MASK) as VMem;
	let table_entry = unsafe { dir_entry_ptr.add(get_addr_element_index(ptr, 0)) };
	let table_entry_value = unsafe { *table_entry };
	if table_entry_value & PAGING_PAGE_PRESENT == 0 {
		return None;
	}
	Some(table_entry)
}

/*
 * Tells whether the given pointer `ptr` is mapped or not.
 */
pub fn is_mapped(vmem: VMem, ptr: *const Void) -> bool {
	resolve(vmem, ptr) != None
}

/*
 * Translates the given virtual address `ptr` to the corresponding physical address. If the address
 * is not mapped, None is returned.
 */
pub fn translate(vmem: VMem, ptr: *const Void) -> Option<*const Void> {
	if let Some(e) = resolve(vmem, ptr) {
		Some((unsafe { *e } & PAGING_ADDR_MASK) as _)
	} else {
		None
	}
}

/*
 * Resolves the entry for the given virtual address `ptr` and returns its flags. This function
 * might return a page directory entry if a large block is present at the corresponding location.
 * If no entry is found, the function returns None.
 */
pub fn get_flags(vmem: VMem, ptr: *const Void) -> Option<u32> {
	if let Some(e) = resolve(vmem, ptr) {
		Some(unsafe { *e } & PAGING_FLAGS_MASK)
	} else {
		None
	}
}

/*
 * Creates an empty page table at index `index` of the page directory.
 */
fn create_table(vmem: MutVMem, index: usize, flags: u32) -> Result<(), ()> {
	debug_assert!(index < 1024);

	let v = buddy::alloc_zero(0, buddy::FLAG_ZONE_TYPE_KERNEL)? as MutVMem;
	unsafe {
		*vmem.add(index) = (v as u32) | flags | PAGING_TABLE_PRESENT;
	}
	Ok(())
}

/*
 * Expands a large block into a page table. This function allocates a new page table and fills it
 * so that the memory mapping keeps the same behaviour.
 */
fn expand_table(vmem: MutVMem, index: usize) -> Result<(), ()> {
	let table_entry = unsafe { vmem.add(index) };
	let table_entry_value = unsafe { *table_entry };
	debug_assert!(table_entry_value & PAGING_TABLE_PRESENT != 0);
	debug_assert!(table_entry_value & PAGING_TABLE_PAGE_SIZE != 0);

	let base_addr = table_entry_value & PAGING_ADDR_MASK;
	let flags = table_entry_value & PAGING_FLAGS_MASK;
	create_table(vmem, index, flags)?;
	for i in 0..1024 {
		let addr = base_addr + (i * memory::PAGE_SIZE) as u32;
		unsafe {
			*table_entry.add(i) = addr | flags | PAGING_PAGE_PRESENT;
		}
	}

	Ok(())
}

/*
 * Maps the the given physical address `physaddr` to the given virtual address `virtaddr` with the
 * given flags. The function forces the FLAG_PAGE_PRESENT flag.
 */
pub fn map(vmem: MutVMem, physaddr: *const Void, virtaddr: *const Void, flags: u32)
	-> Result<(), ()> {
	debug_assert!(util::is_aligned(physaddr, memory::PAGE_SIZE));
	debug_assert!(util::is_aligned(virtaddr, memory::PAGE_SIZE));
	debug_assert!(flags & PAGING_ADDR_MASK == 0);

	let dir_entry_index = get_addr_element_index(virtaddr, 1);
	let dir_entry = unsafe { vmem.add(dir_entry_index) };
	let dir_entry_value = unsafe { *dir_entry };
	if dir_entry_value & PAGING_TABLE_PRESENT == 0 {
		create_table(vmem, dir_entry_index, flags)?;
	} else if dir_entry_value & PAGING_TABLE_PAGE_SIZE == 0 {
		expand_table(vmem, dir_entry_index)?;
	}

	let dir_entry_ptr = (dir_entry_value & PAGING_ADDR_MASK) as MutVMem;
	let table_entry = unsafe { dir_entry_ptr.add(get_addr_element_index(virtaddr, 0)) };
	unsafe {
		*table_entry = (physaddr as u32) | flags;
	}

	Ok(())
}

/*
 * Maps the given physical address `physaddr` to the given virtual address `virtaddr` with the
 * given flags using blocks of 1024 pages (PSE).
 */
pub fn map_pse(_vmem: MutVMem, _physaddr: *const Void, _virtaddr: *const Void, _flags: u32) {
	// TODO
}

/*
 * Maps the given range of physical address `physaddr` to the given range of virtual address
 * `virtaddr`. The range is `pages` pages large.
 */
pub fn map_range(_vmem: MutVMem, _physaddr: *const Void, _virtaddr: *const Void, _pages: usize,
	_flags: u32) {
	// TODO
}

/*
 * Maps the physical address `ptr` to the same address in virtual memory with the given flags
 * `flags`.
 */
pub fn identity(vmem: MutVMem, ptr: *const Void, flags: u32) -> Result<(), ()> {
	map(vmem, ptr, ptr, flags)?;
	Ok(())
}

/*
 * Maps the physical address `ptr` to the same address in virtual memory with the given flags
 * `flags`, using blocks of 1024 pages (PSE).
 */
pub fn identity_pse(_vmem: MutVMem, _ptr: *const Void, _flags: u32) {
	// TODO
}

/*
 * Identity maps a range beginning at physical address `from` with pages `pages` and flags `flags`.
 */
pub fn identity_range(_vmem: MutVMem, _from: *const Void, _pages: usize, _flags: u32) {
	// TODO
}

/*
 * Unmaps the page at virtual address `virtaddr`. The function unmaps only one page, thus if a
 * large block is present at this location (PSE), it shall be split down into a table which shall
 * be filled accordingly.
 */
pub fn unmap(_vmem: MutVMem, _virtaddr: *const Void) {
	// TODO
}

/*
 * Unmaps the given range beginning at virtual address `virtaddr` with size of `pages` pages. Large
 * blocks splitting is supported (PSE).
 */
pub fn unmap_range(_vmem: MutVMem, _virtaddr: *const Void, _pages: usize) {
	// TODO
}

/*
 * Clones the given page directory, allocating copies of every children elements. If the page
 * directory cannot be cloned, the function returns None.
 */
pub fn clone(_vmem: VMem) -> Option<VMem> {
	// TODO
	None
}

/*
 * Flushes the modifications of the given page directory by reloading the Translation Lookaside
 * Buffer (TLB).
 */
pub fn flush(vmem: VMem) {
	unsafe {
		if vmem == (cr3_get() as _) {
			tlb_reload();
		}
	}
}

/*
 * Destroyes the given page directory, including its children elements. If the page directory is
 * begin used, the behaviour is undefined.
 */
pub fn destroy(_vmem: VMem) {
	// TODO
}

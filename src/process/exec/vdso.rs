//! The vDSO (virtual dynamic shared object) is a small shared library that the kernel
//! automatically maps into the memory space of all userspace programs.

use core::cmp::min;
use core::ffi::c_void;
use core::ptr::NonNull;
use core::ptr;
use crate::elf::parser::ELFParser;
use crate::errno::Errno;
use crate::memory::buddy;
use crate::memory;
use crate::process::mem_space::MapConstraint;
use crate::process::mem_space::MapResidence;
use crate::process::mem_space::MemSpace;
use crate::process::mem_space;
use crate::util::container::vec::Vec;
use crate::util::lock::Mutex;
use crate::util::math;
use crate::util::ptr::SharedPtr;

/// Structure storing informations on the vDSO ELF image.
struct VDSO {
	/// The list of pages on which the image is loaded.
	pages: SharedPtr<Vec<NonNull<[u8; memory::PAGE_SIZE]>>>,
	/// The length of the ELF image in bytes.
	len: usize,

	/// The offset to the entry.
	entry_off: usize,
}

/// Informations about the mapped vDSO.
pub struct MappedVDSOInfo {
	/// The pointer to the mapped image.
	pub ptr: NonNull<c_void>,
	/// The pointer to the entry point.
	pub entry: NonNull<c_void>,
}

/// The info of the vDSO. If None, the vDSO is not loaded yet.
static ELF_IMAGE: Mutex<Option<VDSO>> = Mutex::new(None);

/// TODO doc
fn load_image() -> Result<VDSO, Errno> {
	let const_img = include_bytes!("../../../vdso.so");

	// Load image into pages
	let mut pages = Vec::new();
	for i in 0..math::ceil_division(const_img.len(), memory::PAGE_SIZE) {
		// Alloc page
		let ptr = buddy::alloc(0, buddy::FLAG_ZONE_TYPE_KERNEL)?;

		// Copy data
		let off = i * memory::PAGE_SIZE;
		let len = min(memory::PAGE_SIZE, const_img.len() - off);
		unsafe {
			ptr::copy_nonoverlapping(const_img[off..].as_ptr() as *const c_void, ptr, len);
		}

		pages.push(NonNull::new(ptr as *mut [u8; memory::PAGE_SIZE]).unwrap())?;
	}

	// Getting entry point
	let parser = ELFParser::new(const_img.as_slice())?;
	let entry_off = parser.get_header().e_entry as usize;

	Ok(VDSO {
		pages: SharedPtr::new(pages)?,
		len: const_img.len(),

		entry_off,
	})
}

/// Maps the vDSO into the given memory space.
///
/// The function returns a structure containing informations about the mapped image.
pub fn map(mem_space: &mut MemSpace) -> Result<MappedVDSOInfo, Errno> {
	let elf_image_guard = ELF_IMAGE.lock();
	let elf_image = elf_image_guard.get_mut();

	if elf_image.is_none() {
		let img = load_image().expect("Failed to load vDSO");
		*elf_image = Some(img);
	}
	let img = elf_image.as_ref().unwrap();

	// TODO ASLR
	let ptr = mem_space.map(
		MapConstraint::None,
		math::ceil_division(img.len, memory::PAGE_SIZE),
		mem_space::MAPPING_FLAG_USER,
		MapResidence::Static {
			pages: img.pages.clone(),
		}
	)?;

	let entry = unsafe {
		ptr.add(img.entry_off)
	};

	Ok(MappedVDSOInfo {
		ptr: NonNull::new(ptr).unwrap(),
		entry: NonNull::new(entry).unwrap(),
	})
}

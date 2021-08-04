//! The Executable and Linkable Format (ELF) is a format of executable files commonly used in UNIX
//! systems. This module implements a parser allowing to handle this format, including the kernel
//! image itself.

use core::ffi::c_void;
use core::mem::size_of;
use crate::errno::Errno;
use crate::errno;
use crate::memory;
use crate::util;

/// The number of identification bytes in the ELF header.
const EI_NIDENT: usize = 16;

/// Identification bytes offset: File class.
const EI_CLASS: usize = 4;
/// Identification bytes offset: Data encoding.
const EI_DATA: usize = 5;
/// Identification bytes offset: Version.
const EI_VERSION: usize = 6;

/// File's class: Invalid class.
const ELFCLASSNONE: u8 = 0;
/// File's class: 32-bit objects.
const ELFCLASS32: u8 = 1;
/// File's class: 64-bit objects.
const ELFCLASS64: u8 = 2;

/// Data encoding: Invalid data encoding.
const ELFDATANONE: u8 = 0;
/// Data encoding: Little endian.
const ELFDATA2LSB: u8 = 1;
/// Data encoding: Big endian.
const ELFDATA2MSB: u8 = 2;

/// Object file type: No file type.
const ET_NONE: u16 = 0;
/// Object file type: Relocatable file.
const ET_REL: u16 = 1;
/// Object file type: Executable file.
const ET_EXEC: u16 = 2;
/// Object file type: Shared object file.
const ET_DYN: u16 = 3;
/// Object file type: Core file.
const ET_CORE: u16 = 4;
/// Object file type: Processor-specific.
const ET_LOPROC: u16 = 0xff00;
/// Object file type: Processor-specific.
const ET_HIPROC: u16 = 0xffff;

/// Required architecture: AT&T WE 32100.
const EM_M32: u16 = 1;
/// Required architecture: SPARC.
const EM_SPARC: u16 = 2;
/// Required architecture: Intel Architecture.
const EM_386: u16 = 3;
/// Required architecture: Motorola 68000.
const EM_68K: u16 = 4;
/// Required architecture: Motorola 88000.
const EM_88K: u16 = 5;
/// Required architecture: Intel 80860.
const EM_860: u16 = 7;
/// Required architecture: MIPS RS3000 Big-Endian.
const EM_MIPS: u16 = 8;
/// Required architecture: MIPS RS4000 Big-Endian.
const EM_MIPS_RS4_BE: u16 = 10;

/// The section header is inactive.
pub const SHT_NULL: u32 = 0x00000000;
/// The section holds information defined by the program.
pub const SHT_PROGBITS: u32 = 0x00000001;
/// The section holds a symbol table.
pub const SHT_SYMTAB: u32 = 0x00000002;
/// the section holds a string table.
pub const SHT_STRTAB: u32 = 0x00000003;
/// The section holds relocation entries with explicit attends.
pub const SHT_RELA: u32 = 0x00000004;
/// The section holds a symbol hash table.
pub const SHT_HASH: u32 = 0x00000005;
/// The section holds informations for dynamic linking.
pub const SHT_DYNAMIC: u32 = 0x00000006;
/// The section holds informations that marks the file in some way.
pub const SHT_NOTE: u32 = 0x00000007;
/// The section is empty but contains information in its offset.
pub const SHT_NOBITS: u32 = 0x00000008;
/// The section holds relocation entries without explicit attends.
pub const SHT_REL: u32 = 0x00000009;
/// Reserved section type.
pub const SHT_SHLIB: u32 = 0x0000000a;
/// The section holds a symbol table.
pub const SHT_DYNSYM: u32 = 0x0000000b;
/// TODO doc
pub const SHT_INIT_ARRAY: u32 = 0x0000000e;
/// TODO doc
pub const SHT_FINI_ARRAY: u32 = 0x0000000f;
/// TODO doc
pub const SHT_PREINIT_ARRAY: u32 = 0x00000010;
/// TODO doc
pub const SHT_GROUP: u32 = 0x00000011;
/// TODO doc
pub const SHT_SYMTAB_SHNDX: u32 = 0x00000012;
/// TODO doc
pub const SHT_NUM: u32 = 0x00000013;
/// TODO doc
pub const SHT_LOOS: u32 = 0x60000000;

/// The section contains writable data.
pub const SHF_WRITE: u32 = 0x00000001;
/// The section occupies memory during execution.
pub const SHF_ALLOC: u32 = 0x00000002;
/// The section contains executable machine instructions.
pub const SHF_EXECINSTR: u32 = 0x00000004;
/// TODO doc
pub const SHF_MERGE: u32 = 0x00000010;
/// TODO doc
pub const SHF_STRINGS: u32 = 0x00000020;
/// TODO doc
pub const SHF_INFO_LINK: u32 = 0x00000040;
/// TODO doc
pub const SHF_LINK_ORDER: u32 = 0x00000080;
/// TODO doc
pub const SHF_OS_NONCONFORMING: u32 = 0x00000100;
/// TODO doc
pub const SHF_GROUP: u32 = 0x00000200;
/// TODO doc
pub const SHF_TLS: u32 = 0x00000400;
/// TODO doc
pub const SHF_MASKOS: u32 = 0x0ff00000;
/// All bits included in this mask are reserved for processor-specific semantics.
pub const SHF_MASKPROC: u32 = 0xf0000000;
/// TODO doc
pub const SHF_ORDERED: u32 = 0x04000000;
/// TODO doc
pub const SHF_EXCLUDE: u32 = 0x08000000;

/// The symbol's type is not specified.
pub const STT_NOTYPE: u8 = 0;
/// The symbol is associated with a data object, such as a variable, an array, and so on.
pub const STT_OBJECT: u8 = 1;
/// The symbol is associated with a function or other executable code.
pub const STT_FUNC: u8 = 2;
/// The symbol is associated with a section.
pub const STT_SECTION: u8 = 3;
/// TODO doc
pub const STT_FILE: u8 = 4;
/// TODO doc
pub const STT_LOPROC: u8 = 13;
/// TODO doc
pub const STT_HIPROC: u8 = 15;

/// Structure representing an ELF header.
#[repr(C)]
pub struct ELF32ELFHeader {
	/// Identification bytes.
	e_ident: [u8; EI_NIDENT],
	/// Identifies the object file type.
	e_type: u16,
	/// Specifies the required machine type.
	e_machine: u16,
	/// The file's version.
	e_version: u32,
	/// The virtual address of the file's entry point.
	e_entry: u32,
	/// The program header table's file offset in bytes.
	e_phoff: u32,
	/// The section header table's file offset in bytes.
	e_shoff: u32,
	/// Processor-specific flags.
	e_flags: u32,
	/// ELF header's size in bytes.
	e_ehsize: u16,
	/// The size of one entry in the program header table.
	e_phentsize: u16,
	/// The number of entries in the program header table.
	e_phnum: u16,
	/// The size of one entry in the section header table.
	e_shentsize: u16,
	/// The number of entries in the section header table.
	e_shnum: u16,
	/// The section header table index holding the header of the section name string table.
	e_shstrndx: u16,
}

/// Structure representing an ELF section header in memory.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ELF32SectionHeader {
	/// Index in the string table section specifying the name of the section.
	pub sh_name: u32,
	/// The type of the section.
	pub sh_type: u32,
	/// Section flags.
	pub sh_flags: u32,
	/// The address to the section's data in memory during execution.
	pub sh_addr: u32,
	/// The offset of the section's data in the ELF file.
	pub sh_offset: u32,
	/// The size of the section's data in bytes.
	pub sh_size: u32,
	/// Section header table index link.
	pub sh_link: u32,
	/// Extra-informations whose interpretation depends on the section type.
	pub sh_info: u32,
	/// Alignment constraints of the section in memory. `0` or `1` means that the section doesn't
	/// require specific alignment.
	pub sh_addralign: u32,
	/// If the section is a table of entry, this field holds the size of one entry. Else, holds
	/// `0`.
	pub sh_entsize: u32,
}

/// Structure representing an ELF symbol in memory.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ELF32Sym {
	/// Index in the string table section specifying the name of the symbol.
	pub st_name: u32,
	/// The value of the symbol.
	pub st_value: u32,
	/// The size of the symbol.
	pub st_size: u32,
	/// The symbol's type and binding attributes.
	pub st_info: u8,
	/// Holds `0`.
	pub st_other: u8,
	/// The index of the section the symbol is in.
	pub st_shndx: u16,
}

/// Returns a reference to the section with name `name`. If the section is not found, returns None.
/// `sections` is a pointer to the ELF sections of the kernel in the virtual memory.
/// `sections_count` is the number of sections in the kernel.
/// `shndx` is the index of the section containing section names.
/// `entsize` is the size of section entries.
/// `name` is the name of the required section.
pub fn get_section(sections: *const c_void, sections_count: usize, shndx: usize, entsize: usize,
	name: &str) -> Option<&ELF32SectionHeader> {
	debug_assert!(!sections.is_null());
	let names_section = unsafe {
		&*(sections.add(shndx * entsize) as *const ELF32SectionHeader)
	};

	for i in 0..sections_count {
		let hdr = unsafe {
			&*(sections.add(i * entsize) as *const ELF32SectionHeader)
		};
		let n = unsafe {
			util::ptr_to_str(memory::kern_to_virt((names_section.sh_addr + hdr.sh_name) as _))
		};

		if n == name {
			return Some(hdr);
		}
	}

	None
}

/// Iterates over the given section headers list `sections`, calling the given closure `f` for
/// every elements with a reference and the name of the section.
/// `sections` is a pointer to the ELF sections of the kernel in the virtual memory.
/// `sections_count` is the number of sections in the kernel.
/// `shndx` is the index of the section containing section names.
/// `entsize` is the size of section entries.
/// `f` is the closure to be called for each sections.
pub fn foreach_sections<F>(sections: *const c_void, sections_count: usize, shndx: usize,
	entsize: usize, mut f: F) where F: FnMut(&ELF32SectionHeader, &str) {
	let names_section = unsafe {
		&*(sections.add(shndx * entsize) as *const ELF32SectionHeader)
	};

	for i in 0..sections_count {
		let hdr_offset = i * size_of::<ELF32SectionHeader>();
		let hdr = unsafe {
			&*(sections.add(hdr_offset) as *const ELF32SectionHeader)
		};
		let n = unsafe {
			util::ptr_to_str(memory::kern_to_virt((names_section.sh_addr + hdr.sh_name) as _))
		};
		f(hdr, n);
	}
}

/// Returns the name of the symbol at the given offset.
/// `strtab_section` is a reference to the .strtab section, containing symbol names.
/// `offset` is the offset of the symbol in the section.
/// If the offset is invalid or outside of the section, the behaviour is undefined.
pub fn get_symbol_name(strtab_section: &ELF32SectionHeader, offset: u32) -> &'static str {
	debug_assert!(offset < strtab_section.sh_size);

	unsafe {
		util::ptr_to_str(memory::kern_to_virt((strtab_section.sh_addr + offset) as _))
	}
}

/// Returns an Option containing the name of the function for the given instruction pointer. If the
/// name cannot be retrieved, the function returns None.
/// `sections` is a pointer to the ELF sections of the kernel in the virtual memory.
/// `sections_count` is the number of sections in the kernel.
/// `shndx` is the index of the section containing section names.
/// `entsize` is the size of section entries.
/// `inst` is the pointer to the instruction on the virtual memory.
/// If the section `.strtab` doesn't exist, the function returns None.
pub fn get_function_name(sections: *const c_void, sections_count: usize, shndx: usize,
	entsize: usize, inst: *const c_void) -> Option<&'static str> {
	let strtab_section = get_section(sections, sections_count, shndx, entsize, ".strtab")?;
	let mut func_name: Option<&'static str> = None;

	foreach_sections(sections, sections_count, shndx, entsize,
		|hdr: &ELF32SectionHeader, _name: &str| {
			if hdr.sh_type != SHT_SYMTAB {
				return;
			}

			let ptr = memory::kern_to_virt(hdr.sh_addr as _) as *const u8;
			debug_assert!(hdr.sh_entsize > 0);

			let mut i: usize = 0;
			while i < hdr.sh_size as usize {
				let sym = unsafe {
					&*(ptr.add(i) as *const ELF32Sym)
				};

				let value = sym.st_value as usize;
				let size = sym.st_size as usize;
				if (inst as usize) >= value && (inst as usize) < (value + size) {
					if sym.st_name != 0 {
						func_name = Some(get_symbol_name(strtab_section, sym.st_name));
					}

					break;
				}

				i += hdr.sh_entsize as usize;
			}
		});

	func_name
}

/// The ELF parser allows to parse an ELF image and retrieve informations on it.
/// It is especially useful to load a kernel module or a userspace program.
pub struct ELFParser<'a> {
	/// The ELF image.
	image: &'a [u8],
}

impl<'a> ELFParser<'a> {
	// TODO Support 64 bit
	/// Tells whether the ELF image is valid.
	fn check_image(&self) -> bool {
		let signature = [0x7f, b'E', b'L', b'F'];

		if self.image.len() < EI_NIDENT {
			return false;
		}
		if self.image[0..signature.len()] != signature {
			return false;
		}

		// TODO Check relative to current architecture
		if self.image[EI_CLASS] != ELFCLASS32 {
			return false;
		}

		// TODO Check relative to current architecture
		if self.image[EI_DATA] != ELFDATA2LSB {
			return false;
		}

		if self.image.len() < size_of::<ELF32ELFHeader>() {
			return false;
		}
		let ehdr = unsafe { // Safe because the slice is large enough
			&*(&self.image[0] as *const u8 as *const ELF32ELFHeader)
		};

		// TODO Check e_machine
		// TODO Check e_version

		if ehdr.e_ehsize != size_of::<ELF32ELFHeader>() as u16 {
			return false;
		}

		if ehdr.e_phoff + ehdr.e_phentsize as u32 * ehdr.e_phnum as u32
			>= self.image.len() as u32 {
			return false;
		}
		if ehdr.e_shoff + ehdr.e_shentsize as u32 * ehdr.e_shnum as u32
			>= self.image.len() as u32 {
			return false;
		}
		if ehdr.e_shstrndx >= ehdr.e_shnum {
			return false;
		}

		true
	}

	/// Creates a new instance for the given image.
	/// The function checks if the image is valid. If not, the function retuns an error.
	pub fn new(image: &'a [u8]) -> Result<Self, Errno> {
		let p = Self {
			image,
		};

		if p.check_image() {
			Ok(p)
		} else {
			Err(errno::EINVAL)
		}
	}

	// TODO
}

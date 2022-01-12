//! This module implements ACPI related features.
//! The ACPI interface provides informations about the system, allowing to control components such
//! as cooling and powering.
//!
//! The first step in initialization is to read the RSDP table in order to get a pointer to the
//! RSDT, referring to every other available tables.

use core::ffi::c_void;
use core::intrinsics::wrapping_add;
use crate::cpu::CPU;
use crate::cpu;
use crate::memory::dma::DMA;
use crate::memory::dma;
use crate::util;
use data::ACPIData;
use fadt::Fadt;
use madt::Madt;

mod data;
mod fadt;
mod madt;
mod rsdt;

/// Trait representing an ACPI table.
pub trait ACPITable {
	/// Returns the expected signature for the structure.
	fn get_expected_signature() -> [u8; 4];
}

/// An ACPI table header.
#[repr(C)]
pub struct ACPITableHeader {
	/// The signature of the structure.
	signature: [u8; 4],
	/// The length of the structure.
	length: u32,
	/// The revision number of the structure.
	revision: u8,
	/// The checksum to check against all the structure's bytes.
	checksum: u8,
	/// An OEM-supplied string that identifies the OEM.
	oemid: [u8; 6],
	/// The manufacturer model ID.
	oem_table_id: [u8; 8],
	/// OEM revision for supplied OEM table ID.
	oemrevision: u32,
	/// Vendor ID of utility that created the table.
	creator_id: u32,
	/// Revision of utility that created the table.
	creator_revision: u32,
}

impl ACPITableHeader {
	/// Returns the name of the table.
	#[inline(always)]
	pub fn get_signature(&self) -> &[u8; 4] {
		&self.signature
	}

	/// Returns the length of the table.
	#[inline(always)]
	pub fn get_length(&self) -> usize {
		self.length as _
	}

	/// Checks that the table is valid.
	pub fn check(&self) -> bool {
		let length = self.get_length();
		let mut sum: u8 = 0;

		for i in 0..length {
			let byte = unsafe { // Safe since every bytes of `s` are readable.
				*((self as *const Self as *const u8 as usize + i) as *const u8)
			};
			sum = wrapping_add(sum, byte);
		}

		sum == 0
	}
}

/// Boolean value telling whether the century register of the CMOS exist.
static mut CENTURY_REGISTER: bool = false;

/// Tells whether the century register of the CMOS is present.
pub fn is_century_register_present() -> bool {
	unsafe { // Safe because the value is only set once
		CENTURY_REGISTER
	}
}

/// Initializes ACPI.
/// This function must be called only once, at boot.
pub fn init() {
	// Reading ACPI data
	let data = ACPIData::read().unwrap_or_else(| _ | {
		crate::kernel_panic!("Invalid ACPI data!");
	});
	if data.is_none() {
		return;
	}
	let data = data.unwrap();

    if let Some(madt) = data.get_table::<Madt>() {
        let apic_addr = madt.local_apic_addr as *mut c_void;

		// Registering CPU cores
        madt.foreach_entry(| e: &madt::EntryHeader | match e.get_type() {
			madt::ENTRY_PROCESSOR_LOCAL_APIC => {
				let e = unsafe {
					&*(e as *const _ as *const madt::EntryProcessorLocalAPIC)
				};

				cpu::add_core(CPU::new(e.id as _, e.apic_id as _, e.flags)).unwrap_or_else(| _ | {
					crate::kernel_panic!("Failed to register a CPU");
				});
			},

			madt::ENTRY_LOCAL_APIC_ADDRESS_OVERRIDE => {
				if util::ptr_size() == 8 {
					// TODO Re-set apic_addr
					todo!();
				}
			},

			_ => {},
        });

		// TODO doc
        unsafe {
            cpu::apic::set_addr(apic_addr as _);
        }
        dma::register(DMA::new(apic_addr, 1, apic_addr)).unwrap(); // TODO Print proper error msg

		// TODO doc
        madt.foreach_entry(| e: &madt::EntryHeader | {
            match e.get_type() {
                madt::ENTRY_IO_APIC => {
                    let e = unsafe {
                        &*(e as *const _ as *const madt::EntryIOAPIC)
                    };

                    let list_mutex = cpu::list();
                    let mut list_guard = list_mutex.lock();
                    let list = list_guard.get_mut();

                    for i in 0..list.len() {
                        let mut guard = list[i].lock();
                        let cpu = guard.get_mut();

                        if cpu.get_apic_id() == e.io_apic_id as _ {
                            cpu.set_io_apic_addr(Some(e.io_apic_addr as _));
                            break;
                        }
                    }
                },

                _ => {},
            }
        });
    }

	// Setting the century register value
	unsafe { // Safe because the value is only set once
		CENTURY_REGISTER = data.get_table::<Fadt>().map_or(false, | fadt | fadt.century != 0);
	}
}

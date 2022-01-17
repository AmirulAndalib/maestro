//! This file handles interruptions, it provides an interface allowing to register callbacks for
//! each interrupts. Each callback has a priority number and is called in descreasing order.

use core::ffi::c_void;
use core::mem::MaybeUninit;
use crate::cpu::pic;
use crate::cpu;
use crate::errno::Errno;
use crate::idt;
use crate::process::Regs;
use crate::process::tss;
use crate::util::boxed::Box;
use crate::util::container::vec::Vec;
use crate::util::lock::*;

/// The list of interrupt error messages ordered by index of the corresponding interrupt vector.
#[cfg(config_general_arch = "x86")]
static ERROR_MESSAGES: &[&str] = &[
	"Divide-by-zero Error",
	"Debug",
	"Non-maskable Interrupt",
	"Breakpoint",
	"Overflow",
	"Bound Range Exceeded",
	"Invalid Opcode",
	"Device Not Available",
	"Double Fault",
	"Coprocessor Segment Overrun",
	"Invalid TSS",
	"Segment Not Present",
	"Stack-Segment Fault",
	"General Protection Fault",
	"Page Fault",
	"Unknown",
	"x87 Floating-Point Exception",
	"Alignement Check",
	"Machine Check",
	"SIMD Floating-Point Exception",
	"Virtualization Exception",
	"Unknown",
	"Unknown",
	"Unknown",
	"Unknown",
	"Unknown",
	"Unknown",
	"Unknown",
	"Unknown",
	"Unknown",
	"Security Exception",
	"Unknown"
];

/// Returns the error message corresponding to the given interrupt vector index `i`.
fn get_error_message(i: u32) -> &'static str {
	if (i as usize) < ERROR_MESSAGES.len() {
		ERROR_MESSAGES[i as usize]
	} else {
		"Unknown"
	}
}

/// The action to execute after the interrupt handler has returned.
pub enum InterruptResultAction {
	/// Resumes execution of the code where it was interrupted.
	Resume,
	/// Goes back to the kernel loop, waiting for another interruption.
	Loop,
	/// Makes the kernel panic.
	Panic,
}

/// Enumeration telling which action will be executed after an interrupt handler.
pub struct InterruptResult {
	/// Tells whether to skip execution of the next interrupt handlers (with lower priority).
	skip_next: bool,
	/// The action to execute after the handler. The last handler decides which action to execute
	/// unless the `skip_next` variable is set to `true`.
	action: InterruptResultAction,
}

impl InterruptResult {
	/// Creates a new instance.
	pub fn new(skip_next: bool, action: InterruptResultAction) -> Self {
		Self {
			skip_next,
			action,
		}
	}
}

/// Structure wrapping a callback to insert it into a linked list.
struct CallbackWrapper {
	/// The priority associated with the callback. Higher value means higher priority
	priority: u32,

	/// The callback
	/// First argument: `id` is the id of the interrupt.
	/// Second argument: `code` is an optional code associated with the interrupt. If no code
	/// is given, the value is `0`.
	/// Third argument: `regs` the values of the registers when the interruption was triggered.
	/// Fourth argument: `ring` tells the ring at which the code was running.
	/// The return value tells which action to perform next.
	callback: Box<dyn FnMut(u32, u32, &Regs, u32) -> InterruptResult>,
}

/// Structure used to detect whenever the object owning the callback is destroyed, allowing to
/// unregister it automatically.
#[must_use]
pub struct CallbackHook {
	/// The id of the interrupt the callback is bound to.
	id: u8,
	/// The priority of the callback.
	priority: u32,

	/// The pointer of the callback.
	ptr: *const c_void,
}

impl CallbackHook {
	/// Creates a new instance.
	fn new(id: u8, priority: u32, ptr: *const c_void) -> Self {
		Self {
			id,
			priority,

			ptr,
		}
	}
}

impl Drop for CallbackHook {
	fn drop(&mut self) {
		remove_callback(self.id, self.priority, self.ptr);
	}
}

/// List containing vectors that store callbacks for every interrupt watchdogs.
static mut CALLBACKS: MaybeUninit<[IntMutex<Vec<CallbackWrapper>>; idt::ENTRIES_COUNT as _]>
	= MaybeUninit::uninit();

/// Initializes the events handler.
/// This function must be called only once when booting.
pub fn init() {
	let callbacks = unsafe { // Safe because called only once
		CALLBACKS.assume_init_mut()
	};

	for c in callbacks {
		*c.lock().get_mut() = Vec::new();
	}
}

/// Registers the given callback and returns a reference to it.
/// `id` is the id of the interrupt to watch.
/// `priority` is the priority for the callback. Higher value means higher priority.
/// `callback` is the callback to register.
///
/// If the `id` is invalid or if an allocation fails, the function shall return an error.
pub fn register_callback<T>(id: u8, priority: u32, callback: T) -> Result<CallbackHook, Errno>
	where T: 'static + FnMut(u32, u32, &Regs, u32) -> InterruptResult {
	debug_assert!((id as usize) < idt::ENTRIES_COUNT);

	idt::wrap_disable_interrupts(|| {
		let mut guard = unsafe {
			CALLBACKS.assume_init_mut()
		}[id as usize].lock();
		let vec = &mut guard.get_mut();

		let index = {
			let r = vec.binary_search_by(| x | {
				x.priority.cmp(&priority)
			});

			if let Err(l) = r {
				l
			} else {
				r.unwrap()
			}
		};

		let b = Box::new(callback)?;
		let ptr = b.as_ptr();
		vec.insert(index, CallbackWrapper {
			priority,
			callback: b,
		})?;

		Ok(CallbackHook::new(id, priority, ptr as _))
	})
}

/// Removes the callback with id `id`, priority `priority` and pointer `ptr`.
fn remove_callback(id: u8, priority: u32, ptr: *const c_void) {
	let mut guard = unsafe {
		CALLBACKS.assume_init_mut()
	}[id as usize].lock();
	let vec = &mut guard.get_mut();

	let res = vec.binary_search_by(| x | {
		x.priority.cmp(&priority)
	});
	if let Ok(index) = res {
		let mut i = index;

		while i < vec.len() && vec[i].priority == priority {
			if vec[i].callback.as_ptr() as *const c_void == ptr {
				vec.remove(i);
				break;
			}

			i += 1;
		}
	}
}

/// Unlocks the callback vector with id `id`. This function is to be used in case of an event
/// callback that never returns.
/// It must be called from the same CPU core as the one that locked the mutex since unlocking
/// changes the interrupt flag.
/// This function is marked as unsafe since it may lead to concurrency issues if not used properly.
#[no_mangle]
pub unsafe extern "C" fn unlock_callbacks(id: usize) {
	CALLBACKS.assume_init_mut()[id as usize].unlock();
}

/// This function is called whenever an interruption is triggered.
/// `id` is the identifier of the interrupt type. This value is architecture-dependent.
/// `code` is an optional code associated with the interrupt. If the interrupt type doesn't have a
/// code, the value is `0`.
/// `regs` is the state of the registers at the moment of the interrupt.
/// `ring` tells the ring at which the code was running.
#[no_mangle]
pub extern "C" fn event_handler(id: u32, code: u32, ring: u32, regs: &Regs) {
	let action = {
		let mut guard = unsafe {
			&mut CALLBACKS.assume_init_mut()[id as usize]
		}.lock();
		let callbacks = guard.get_mut();

		let mut last_action = {
			if (id as usize) < ERROR_MESSAGES.len() {
				InterruptResultAction::Panic
			} else {
				InterruptResultAction::Resume
			}
		};

		for i in 0..callbacks.len() {
			let result = (callbacks[i].callback)(id, code, regs, ring);
			last_action = result.action;
			if result.skip_next {
				break;
			}
		}

		last_action
	};

	match action {
		InterruptResultAction::Resume => {},

		InterruptResultAction::Loop => {
			pic::end_of_interrupt(id as _);
			// FIXME: Use of loop action before TSS init shall result in undefined behaviour

			unsafe {
				cpu::loop_reset(tss::get().esp0 as _);
			}
		},

		InterruptResultAction::Panic => {
			crate::kernel_panic!(get_error_message(id), code);
		},
	}
}

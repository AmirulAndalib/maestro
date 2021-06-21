//! The role of the process scheduler is to interrupt the currently running process periodicaly
//! to switch to another process that is in running state. The interruption is fired by the PIT
//! on IDT0.
//!
//! A scheduler cycle is a period during which the scheduler iterates through every processes.
//! The scheduler works by assigning a number of quantum for each process, based on the number of
//! running processes and their priority.
//! This number represents the number of ticks during which the process keeps running until
//! switching to the next process.

use core::cmp::max;
use core::ffi::c_void;
use crate::cpu;
use crate::errno::Errno;
use crate::event::{Callback, CallbackHook, InterruptResult};
use crate::event;
use crate::gdt;
use crate::memory::malloc;
use crate::memory::stack;
use crate::memory;
use crate::process::Process;
use crate::process::pid::Pid;
use crate::process::tss;
use crate::process;
use crate::util::Regs;
use crate::util::container::vec::Vec;
use crate::util::lock::mutex::*;
use crate::util::math;
use crate::util::ptr::SharedPtr;
use crate::util;

/// The size of the temporary stack for context switching.
const TMP_STACK_SIZE: usize = memory::PAGE_SIZE;
/// The number of quanta for the process with the average priority.
const AVERAGE_PRIORITY_QUANTA: usize = 10;
/// The number of quanta for the process with the maximum priority.
const MAX_PRIORITY_QUANTA: usize = 30;

extern "C" {
	/// This function switches to a userspace context.
	/// `regs` is the structure of registers to restore to resume the context.
	/// `data_selector` is the user data segment selector.
	/// `code_selector` is the user code segment selector.
	fn context_switch(regs: &Regs, data_selector: u16, code_selector: u16) -> !;
	/// This function switches to a kernelspace context.
	/// `regs` is the structure of registers to restore to resume the context.
	fn context_switch_kernel(regs: &Regs) -> !;
}

/// The structure containing the context switching data.
struct ContextSwitchData {
	///  The process to switch to.
	proc: SharedPtr::<Mutex::<Process>>,
}

/// Scheduler ticking callback.
pub struct TickCallback {
	/// A reference to the scheduler.
	scheduler: SharedPtr<Mutex<Scheduler>>,
}

impl Callback for TickCallback {
	fn call(&mut self, _id: u32, _code: u32, regs: &util::Regs, ring: u32) -> InterruptResult {
		Scheduler::tick(&mut self.scheduler, regs, ring);
	}
}

/// The structure representing the process scheduler.
pub struct Scheduler {
	/// A vector containing the temporary stacks for each CPU cores.
	tmp_stacks: Vec<malloc::Alloc<u8>>,

	/// The ticking callback hook.
	/// The ticking callback is called at a regular interval to make the scheduler work.
	tick_callback_hook: Option<CallbackHook>,
	/// The total number of ticks since the instanciation of the scheduler.
	total_ticks: u64,

	/// The list of all processes.
	processes: Vec<SharedPtr<Mutex<Process>>>,
	/// The currently running process.
	curr_proc: Option<SharedPtr<Mutex<Process>>>,

	/// The sum of all priorities, used to compute the average priority.
	priority_sum: usize,
	/// The priority of the processs which has the current highest priority.
	priority_max: usize,

	/// The current process cursor on the `processes` list.
	cursor: usize,
}

impl Scheduler {
	/// Creates a new instance of scheduler.
	pub fn new(cores_count: usize) -> Result<SharedPtr::<Mutex::<Self>>, Errno> {
		let mut tmp_stacks = Vec::new();
		for _ in 0..cores_count {
			tmp_stacks.push(malloc::Alloc::new_default(TMP_STACK_SIZE)?)?;
		}

		let mut s = SharedPtr::new(Mutex::new(Self {
			tmp_stacks,

			tick_callback_hook: None,
			total_ticks: 0,

			processes: Vec::<SharedPtr::<Mutex::<Process>>>::new(),
			curr_proc: None,

			priority_sum: 0,
			priority_max: 0,

			cursor: 0,
		}))?;

		{
			let callback = TickCallback {
				scheduler: s.clone(),
			};
			let mut guard = MutexGuard::new(&mut s);
			let scheduler = guard.get_mut();
			scheduler.tick_callback_hook = Some(event::register_callback(32, 0, callback)?);
		}
		Ok(s)
	}

	/// Returns the process with PID `pid`. If the process doesn't exist, the function returns None.
	pub fn get_by_pid(&mut self, pid: Pid) -> Option::<SharedPtr::<Mutex::<Process>>> {
		// TODO Optimize
		for i in 0..self.processes.len() {
			let proc = &mut self.processes[i];

			if proc.lock().get().get_pid() == pid {
				return Some(proc.clone());
			}
		}

		None
	}

	/// Returns the current running process. If no process is running, the function returns None.
	pub fn get_current_process(&mut self) -> Option::<SharedPtr::<Mutex::<Process>>> {
		self.curr_proc.as_ref().cloned()
	}

	/// Updates the scheduler's heuristic with the new priority of a process.
	/// `old` is the old priority of the process.
	/// `new` is the newe priority of the process.
	/// The function doesn't need to know the process which has been updated since it updates
	/// global informations.
	pub fn update_priority(&mut self, old: usize, new: usize) {
		self.priority_sum = self.priority_sum - old + new;
		if old >= self.priority_max {
			self.priority_max = new;
		}
	}

	/// Adds a process to the scheduler.
	pub fn add_process(&mut self, process: Process)
		-> Result<SharedPtr::<Mutex::<Process>>, Errno> {
		let priority = process.get_priority();
		let ptr = SharedPtr::new(Mutex::new(process))?;
		self.processes.push(ptr.clone())?;
		self.update_priority(0, priority);

		Ok(ptr)
	}

	// TODO Remove process (don't forget to update the priority)

	/// Returns the average priority of a process.
	fn get_average_priority(&self) -> usize {
		self.priority_sum / self.processes.len()
	}

	/// Returns the number of quantum for the given priority.
	fn get_quantum_count(&self, priority: usize) -> usize {
		let n = math::integer_linear_interpolation::<isize>(priority as _,
			self.get_average_priority() as _,
			self.priority_max as _,
			AVERAGE_PRIORITY_QUANTA as _,
			MAX_PRIORITY_QUANTA as _);
		max(1, n) as _
	}

	/// Tells whether the given process can be run.
	/// `i` is the index of the process in the processes list.
	fn can_run(&self, i: usize) -> bool {
		let mut mutex = self.processes[i].clone();
		let guard = MutexGuard::new(&mut mutex);
		let process = guard.get();

		if process.get_state() == process::State::Running {
			let cursor_priority = process.priority;
			process.quantum_count < self.get_quantum_count(cursor_priority)
		} else {
			false
		}
	}

	/// Returns the next process to run.
	fn get_next_process(&mut self) -> Option::<&mut SharedPtr::<Mutex::<Process>>> {
		if !self.processes.is_empty() {
			let processes_count = self.processes.len();
			let mut i = self.cursor;
			let mut j = 0;
			while j < processes_count && !self.can_run(i) {
				i = (i + 1) % processes_count;
				j += 1;
			}

			if self.cursor != i || processes_count == 1 {
				self.processes[self.cursor].lock().get_mut().quantum_count = 0;
			}
			self.cursor = i;

			if self.can_run(self.cursor) {
				Some(&mut self.processes[self.cursor])
			} else {
				None
			}
		} else {
			None
		}
	}

	/// Ticking the scheduler. This function saves the data of the currently running process, then
	/// switches to the next process to run.
	/// `mutex` is the scheduler's mutex.
	/// `regs` is the state of the registers from the paused context.
	/// `ring` is the ring of the paused context.
	fn tick(mutex: &mut Mutex<Self>, regs: &util::Regs, ring: u32) -> ! {
		let mut guard = mutex.lock();
		let scheduler = guard.get_mut();

		scheduler.total_ticks += 1;

		if let Some(mut curr_proc) = scheduler.get_current_process() {
			let mut guard = MutexGuard::new(&mut curr_proc);
			let curr_proc = guard.get_mut();

			curr_proc.regs = *regs;
			curr_proc.syscalling = ring < 3;
		}

		if let Some(next_proc) = scheduler.get_next_process() {
			scheduler.curr_proc = Some(next_proc.clone());

			let f = | data | {
				let (syscalling, regs) = {
					let data = unsafe {
						&mut *(data as *mut ContextSwitchData)
					};
					let mut guard = MutexGuard::new(&mut data.proc);
					let proc = guard.get_mut();
					proc.quantum_count += 1;

					let tss = tss::get();
					tss.ss0 = gdt::KERNEL_DATA_OFFSET as _;
					tss.ss = gdt::USER_DATA_OFFSET as _;
					tss.esp0 = proc.kernel_stack as _;
					proc.mem_space.bind();

					let eip = proc.regs.eip;
					let vmem = proc.mem_space.get_vmem();
					debug_assert!(vmem.translate(eip as _).is_some());

					(proc.is_syscalling(), proc.regs)
				};

				if syscalling {
					unsafe {
						context_switch_kernel(&regs);
					}
				} else {
					unsafe {
						context_switch(&regs,
							(gdt::USER_DATA_OFFSET | 3) as _,
							(gdt::USER_CODE_OFFSET | 3) as _);
					}
				}
			};

			let tmp_stack = unsafe {
				scheduler.tmp_stacks[cpu::get_current() as _].as_ptr_mut() as *mut c_void
			};
			let ctx_switch_data = ContextSwitchData {
				proc: scheduler.curr_proc.as_mut().unwrap().clone(),
			};

			drop(guard);
			unsafe {
				stack::switch(tmp_stack, f, ctx_switch_data).unwrap();
			}
			crate::enter_loop();
		} else if cfg!(config_general_scheduler_end_panic) {
			kernel_panic!("No process remaining to run!");
		} else {
			crate::halt();
		}
	}

	/// Returns the total number of ticks since the instanciation of the scheduler.
	pub fn get_total_ticks(&self) -> u64 {
		self.total_ticks
	}
}

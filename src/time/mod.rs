//! This module handles time-releated features.
//!
//! A clock source is an object that allow to get the current timestamp.
//! A timer allows to interrupt the system at a specified frequency. Delay functions work using those.

pub mod timer;

use crate::errno::Errno;
use crate::util::boxed::Box;
use crate::util::container::vec::Vec;
use crate::util::lock::Mutex;

/// Type representing a timestamp.
pub type Timestamp = u32;

/// Trait representing a source able to provide the current timestamp.
pub trait ClockSource {
	/// The name of the source.
	fn get_name(&self) -> &str;
	/// Returns the current timestamp in seconds.
	fn get_time(&mut self) -> Timestamp;
}

// TODO Order by name to allow binary search
/// Vector containing all the clock sources.
static CLOCK_SOURCES: Mutex<Vec<Box<dyn ClockSource>>> = Mutex::new(Vec::new());

/// Returns a reference to the list of clock sources.
pub fn get_clock_sources() -> &'static Mutex<Vec<Box<dyn ClockSource>>> {
	&CLOCK_SOURCES
}

/// Adds the new clock source to the clock sources list.
pub fn add_clock_source<T: 'static + ClockSource>(source: T) -> Result<(), Errno> {
	let mut guard = CLOCK_SOURCES.lock();
	let sources = guard.get_mut();
	sources.push(Box::new(source)?)?;
	Ok(())
}

/// Removes the clock source with the given name.
/// If the clock source doesn't exist, the function does nothing.
pub fn remove_clock_source(name: &str) {
	let mut guard = CLOCK_SOURCES.lock();
	let sources = guard.get_mut();

	for i in 0..sources.len() {
		if sources[i].get_name() == name {
			sources.remove(i);
			return;
		}
	}
}

/// Returns the current timestamp from the preferred clock source.
/// If no clock source is available, the function returns None.
pub fn get() -> Option<Timestamp> {
	let mut guard = CLOCK_SOURCES.lock();
	let sources = guard.get_mut();

	if !sources.is_empty() {
		let cmos = &mut sources[0]; // TODO Select the preferred source
		Some(cmos.get_time())
	} else {
		None
	}
}

/// Makes the CPU wait for at least `n` milliseconds.
pub fn mdelay(_n: u32) {
	// TODO
}

/// Makes the CPU wait for at least `n` microseconds.
pub fn udelay(_n: u32) {
	// TODO
}

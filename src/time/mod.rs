//! This module handles time-releated features.
//! The kernel stores a list of clock sources. A clock source is an object that allow to get the
//! current timestamp.

use crate::errno::Errno;
use crate::util::boxed::Box;
use crate::util::container::vec::Vec;
use crate::util::lock::*;

/// Type representing a timestamp in seconds. Equivalent to POSIX's `time_t`.
pub type Timestamp = u64;
/// Type representing a timestamp in microseconds. Equivalent to POSIX's `suseconds_t`.
pub type UTimestamp = u64;
/// Type representing an elapsed number of ticks. Equivalent to POSIX's `clock_t`.
pub type Clock = u32;

/// Trait to be implement on a structure describing a moment in time.
pub trait TimeUnit {
	/// Creates the structure from the given timestamp in nanoseconds.
	fn from_nano(timestamp: u64) -> Self;
}

/// POSIX structure representing a timestamp.
#[derive(Clone, Default)]
#[repr(C)]
pub struct Timeval {
	/// Seconds
	pub tv_sec: Timestamp,
	/// Microseconds
	pub tv_usec: UTimestamp,
}

impl TimeUnit for Timeval {
	fn from_nano(timestamp: u64) -> Self {
		let sec = timestamp / 1000000000;
		let usec = (timestamp % 1000000000) / 1000;

		Self {
			tv_sec: sec,
			tv_usec: usec,
		}
	}
}

/// Same as `Timeval`, but with nanosecond precision.
#[derive(Clone, Default)]
#[repr(C)]
pub struct Timespec {
	/// Seconds
	pub tv_sec: Timestamp,
	/// Nanoseconds
	pub tv_nsec: u32,
}

impl TimeUnit for Timespec {
	fn from_nano(timestamp: u64) -> Self {
		let sec = timestamp / 1000000000;
		let nsec = timestamp % 1000000000;

		Self {
			tv_sec: sec,
			tv_nsec: nsec as _,
		}
	}
}

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
/// TODO specify the time unit
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

/// Returns the current timestamp from the given clock `clk`.
/// If the clock doesn't exist, the function returns None.
pub fn get_struct<T: TimeUnit>(_clk: &[u8]) -> Option<T> {
	// TODO use the given clock
	// TODO use the correct unit
	Some(T::from_nano(get()?))
}

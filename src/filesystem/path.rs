/// This module handles files path.

use core::ops::Index;
use core::ops::IndexMut;
use crate::errno::Errno;
use crate::util::FailableClone;
use crate::util::container::string::String;
use crate::util::container::vec::Vec;

// TODO In path, check that every component matches the maximum allowed length

/// The character used as a path separator.
pub const PATH_SEPARATOR: char = '/';

/// A structure representing a path to a file.
pub struct Path {
	/// Tells whether the path is absolute or relative.
	absolute: bool,
	/// An array containing the different parts of the path which are separated with `/`.
	parts: Vec<String>,
}

impl Path {
	/// Creates a new instance to the root directory.
	pub fn root() -> Self {
		Self {
			absolute: true,
			parts: Vec::new(),
		}
	}

	/// Creates a new instance from string.
	pub fn from_string(path: &str) -> Result<Self, Errno> {
		let mut parts = Vec::new();
		for p in path.split(PATH_SEPARATOR) {
			if !p.is_empty() {
				parts.push(String::from(p)?)?;
			}
		}

		Ok(Self {
			absolute: path.chars().next().unwrap() == PATH_SEPARATOR,
			parts: parts,
		})
	}

	/// Tells whether the path is absolute or not.
	pub fn is_absolute(&self) -> bool {
		self.absolute
	}

	/// Returns the number of elements in the path, namely, the number of elements separated by
	/// `/`.
	pub fn get_elements_count(&self) -> usize {
		self.parts.len()
	}

	/// Converts the path into a String and returns it.
	pub fn as_string(&self) -> Result<String, Errno> {
		let mut s = String::new();
		if self.absolute {
			s.push('/')?;
		}

		for (i, p) in self.parts.iter().enumerate() {
			s.push_str(p)?;
			if i + 1 < self.parts.len() {
				s.push('/')?;
			}
		}

		Ok(s)
	}

	/// Pushes the given filename `filename` onto the path.
	pub fn push(&mut self, filename: String) -> Result<(), Errno> {
		self.parts.push(filename)
	}

	/// Pops the filename on top of the path.
	pub fn pop(&mut self) {
		self.parts.pop();
	}

	/// Reduces the path, removing all useless `.` and `..`.
	pub fn reduce(&mut self) -> Result<(), Errno> {
		let mut i = 0;
		while i < self.parts.len() {
			let part = &self.parts[i];
			if part == "." {
				self.parts.remove(i);
			} else if part == ".." && self.absolute && i == 0 {
				self.parts.remove(i);
			} else if part == ".." && i > 0 && self.parts[i - 1] != ".." {
				self.parts.remove(i);
				self.parts.remove(i - 1);
				i -= 1;
			} else {
				i += 1;
			}
		}

		if !self.absolute && self.parts.is_empty() {
			self.parts.push(String::from(".")?)?;
		}

		Ok(())
	}

	/// Concats the current path with another path `other` to create a new path. The path is not
	/// automaticaly reduced.
	pub fn concat(&self, other: &Self) -> Result<Self, Errno> {
		let mut self_parts = self.parts.failable_clone()?;
		let mut other_parts = other.parts.failable_clone()?;
		self_parts.append(&mut other_parts)?;
		Ok(Self {
			absolute: self.absolute,
			parts: self_parts,
		})
	}
}

impl FailableClone for Path {
	fn failable_clone(&self) -> Result<Self, Errno> {
		Ok(Self {
			absolute: self.absolute,
			parts: self.parts.failable_clone()?,
		})
	}
}

impl Index<usize> for Path {
	type Output = String;

	#[inline]
	fn index(&self, index: usize) -> &Self::Output {
		&self.parts[index]
	}
}

impl IndexMut<usize> for Path {
	#[inline]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		&mut self.parts[index]
	}
}

// TODO Iterator

#[cfg(test)]
mod test {
	use super::*;

	#[test_case]
	fn path_absolute0() {
		assert!(Path::from_string("/").unwrap().is_absolute());
	}

	#[test_case]
	fn path_absolute1() {
		assert!(Path::from_string("/.").unwrap().is_absolute());
	}

	#[test_case]
	fn path_absolute2() {
		assert!(!Path::from_string(".").unwrap().is_absolute());
	}

	#[test_case]
	fn path_absolute3() {
		assert!(!Path::from_string("..").unwrap().is_absolute());
	}

	#[test_case]
	fn path_absolute4() {
		assert!(!Path::from_string("./").unwrap().is_absolute());
	}

	#[test_case]
	fn path_reduce0() {
		let mut path = Path::from_string("/.").unwrap();
		path.reduce().unwrap();
		assert_eq!(path.as_string().unwrap(), "/");
	}

	#[test_case]
	fn path_reduce1() {
		let mut path = Path::from_string("/..").unwrap();
		path.reduce().unwrap();
		assert_eq!(path.as_string().unwrap(), "/");
	}

	#[test_case]
	fn path_reduce2() {
		let mut path = Path::from_string("./").unwrap();
		path.reduce().unwrap();
		assert_eq!(path.as_string().unwrap(), ".");
	}

	#[test_case]
	fn path_reduce3() {
		let mut path = Path::from_string("../").unwrap();
		path.reduce().unwrap();
		assert_eq!(path.as_string().unwrap(), "..");
	}

	#[test_case]
	fn path_reduce4() {
		let mut path = Path::from_string("../bleh").unwrap();
		path.reduce().unwrap();
		assert_eq!(path.as_string().unwrap(), "../bleh");
	}

	#[test_case]
	fn path_reduce5() {
		let mut path = Path::from_string("../bleh/..").unwrap();
		path.reduce().unwrap();
		assert_eq!(path.as_string().unwrap(), "..");
	}

	#[test_case]
	fn path_reduce6() {
		let mut path = Path::from_string("../bleh/../bluh").unwrap();
		path.reduce().unwrap();
		assert_eq!(path.as_string().unwrap(), "../bluh");
	}

	#[test_case]
	fn path_reduce7() {
		let mut path = Path::from_string("/bleh/../bluh").unwrap();
		path.reduce().unwrap();
		assert_eq!(path.as_string().unwrap(), "/bluh");
	}

	#[test_case]
	fn path_reduce8() {
		let mut path = Path::from_string("/bleh/../../bluh").unwrap();
		path.reduce().unwrap();
		assert_eq!(path.as_string().unwrap(), "/bluh");
	}

	// TODO test concat
}

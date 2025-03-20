/*
 * Copyright 2024 Luc Lenôtre
 *
 * This file is part of Maestro.
 *
 * Maestro is free software: you can redistribute it and/or modify it under the
 * terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or (at your option) any later
 * version.
 *
 * Maestro is distributed in the hope that it will be useful, but WITHOUT ANY
 * WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
 * A PARTICULAR PURPOSE. See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * Maestro. If not, see <https://www.gnu.org/licenses/>.
 */

//! A non-concurrent, intrusive, doubly-linked list implementation.
//!
//! Intrusive linked-lists are useful in context where memory allocations should be avoided.
//! Otherwise, prefer using other collections.

use crate::ptr::arc::Arc;
use core::{
	cell::UnsafeCell,
	marker::{PhantomData, PhantomPinned},
	ptr,
	ptr::NonNull,
};

/// A non-concurrent, intrusive, doubly-linked list node.
///
/// Most operations on this structure are unsafe as they cannot fit Rust's borrowing rules.
#[derive(Default)]
pub struct ListNode {
	prev: UnsafeCell<Option<NonNull<ListNode>>>,
	next: UnsafeCell<Option<NonNull<ListNode>>>,
	_pin: PhantomPinned,
}

impl ListNode {
	/// Returns the container of `self`.
	///
	/// `inner_off` is the offset of `self` inside of `T`.
	///
	/// # Safety
	///
	/// If `self` is not a field inside of `T`, or if `inner_off` is invalid, the behaviour is
	/// undefined.
	#[inline]
	pub unsafe fn container<T>(&self, inner_off: usize) -> &T {
		&*(self as *const Self).byte_sub(inner_off).cast::<T>()
	}

	/// Returns a reference to the previous element.
	#[inline]
	pub fn prev(&self) -> Option<&Self> {
		unsafe { (*self.prev.get()).map(|n| n.as_ref()) }
	}

	/// Returns a reference to the next element.
	#[inline]
	pub fn next(&self) -> Option<&Self> {
		unsafe { (*self.next.get()).map(|n| n.as_ref()) }
	}

	/// Inserts `self` after `node` in the list.
	///
	/// # Safety
	///
	/// It is the caller's responsibility to ensure concurrency and consistency are handled
	/// correctly.
	pub unsafe fn insert_after(&self, mut node: NonNull<ListNode>) {
		// Insert in the new list
		*self.prev.get() = Some(node);
		*self.next.get() = *node.as_ref().next.get();
		// Link back
		*node.as_mut().next.get() = Some(NonNull::from(self));
		if let Some(next) = self.next() {
			*next.prev.get() = Some(NonNull::from(self));
		}
	}

	/// Unlinks `self` from its list. If not in a list, the function does nothing
	///
	/// # Safety
	///
	/// It is the caller's responsibility to ensure concurrency and consistency are handled
	/// correctly.
	pub unsafe fn unlink(&self) {
		let prev = (*self.prev.get()).take();
		let next = (*self.next.get()).take();
		if let Some(mut prev) = prev {
			*prev.as_mut().next.get() = next;
		}
		if let Some(mut next) = next {
			*next.as_mut().prev.get() = prev;
		}
	}
}

/// The base of a non-concurrent, intrusive, doubly linked list.
///
/// The elements inside the list have to reside in an [`Arc`]. This prevents ownership issues and
/// preserves soundness by disallowing mutability on the inner list node.
///
/// This structure uses mutability in order to enforce locking in concurrent contexts.
///
/// Inside, the linked list forms a cycle to allow iterating both ways.
///
/// An instance can be created with [`crate::list`].
///
/// When dropped, if the list is not empty, the remaining nodes are kept as is in a circular list.
/// If no other reference to the nodes exist, the content of the list is **leaked**.
pub struct List<T, const OFF: usize> {
	// We are using `prev` as the tail and `next` as the head
	base: ListNode,
	_data: PhantomData<T>,
}

/// Initialize a new list.
///
/// This macro can be used in a `const` context.
#[macro_export]
macro_rules! list {
	($ty:ty, $field:ident) => {{
		const OFF: usize = core::mem::offset_of!($ty, $field);
		$crate::collections::list::List::<$ty, OFF>::_new()
	}};
}

impl<T, const OFF: usize> List<T, OFF> {
	/// Use [`crate::list`] instead!
	pub const fn _new() -> Self {
		Self {
			base: ListNode {
				prev: UnsafeCell::new(None),
				next: UnsafeCell::new(None),
				_pin: PhantomPinned,
			},
			_data: PhantomData,
		}
	}

	/// Returns an iterator over the list.
	pub fn iter(&mut self) -> Iter<'_, T, OFF> {
		Iter {
			list: NonNull::from(&mut *self),

			start: self.base.next().unwrap_or(&self.base),
			end: &self.base,
		}
	}

	fn insert_front_impl(&mut self, mut node: NonNull<ListNode>) {
		let base_node = NonNull::from(&mut self.base);
		let node_ref = unsafe { node.as_mut() };
		if self.base.next().is_some() {
			// There is already an element in the list
			unsafe {
				node_ref.insert_after(base_node);
			}
		} else {
			// There is element in the list: insert the first element and make a cycle
			*node_ref.prev.get_mut() = Some(base_node);
			*node_ref.next.get_mut() = Some(base_node);
			*self.base.prev.get_mut() = Some(node);
			*self.base.next.get_mut() = Some(node)
		}
	}

	/// Inserts `val` at the first position of the list.
	pub fn insert_front(&mut self, val: Arc<T>) {
		unsafe {
			let inner = NonNull::from(&*Arc::into_raw(val));
			let node = inner.byte_add(OFF).cast::<ListNode>();
			self.insert_front_impl(node);
		}
	}

	/// Removes the first element of the list and returns it, if any.
	pub fn remove_front(&mut self) -> Option<Arc<T>> {
		let head = (*self.base.next.get_mut())?;
		let mut cursor = Cursor {
			list: NonNull::from(self),
			node: head,

			phantom: PhantomData,
		};
		Some(cursor.remove())
	}
}

impl<T, const OFF: usize> Drop for List<T, OFF> {
	fn drop(&mut self) {
		// Prevent dangling reference
		unsafe {
			self.base.unlink();
		}
	}
}

/// Cursor over an element in a [`List`].
pub struct Cursor<'l, T: 'l, const OFF: usize> {
	list: NonNull<List<T, OFF>>,
	node: NonNull<ListNode>,

	phantom: PhantomData<&'l mut T>,
}

impl<'l, T: 'l, const OFF: usize> Cursor<'l, T, OFF> {
	/// Returns the cursor's node.
	#[inline]
	pub fn node(&self) -> &ListNode {
		unsafe { self.node.as_ref() }
	}

	/// Returns a reference to the node's value.
	#[inline]
	pub fn value(&self) -> &T {
		unsafe { self.node().container(OFF) }
	}

	/// Removes the element from the list, returning the value as an [`Arc`].
	pub fn remove(&mut self) -> Arc<T> {
		let node = self.node();
		unsafe {
			node.unlink();
			Arc::from_raw(node.container(OFF))
		}
	}

	/// Moves the node to the beginning of the list.
	///
	/// This is useful when the list is used as an LRU.
	pub fn lru_promote(&mut self) {
		unsafe {
			self.node().unlink();
			self.list.as_mut().insert_front_impl(self.node);
		}
	}
}

/// Double-ended iterator over a [`List`], returning a [`Cursor`] for each element.
pub struct Iter<'l, T: 'l, const OFF: usize> {
	list: NonNull<List<T, OFF>>,

	start: &'l ListNode,
	end: &'l ListNode,
}

impl<'l, T: 'l, const OFF: usize> Iterator for Iter<'l, T, OFF> {
	type Item = Cursor<'l, T, OFF>;

	fn next(&mut self) -> Option<Self::Item> {
		if ptr::eq(self.start, self.end) {
			return None;
		}
		let node = self.start;
		self.start = node.next()?;
		Some(Cursor {
			list: self.list,
			node: NonNull::from(node),

			phantom: PhantomData,
		})
	}
}

impl<'l, T: 'l, const OFF: usize> DoubleEndedIterator for Iter<'l, T, OFF> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if ptr::eq(self.start, self.end) {
			return None;
		}
		let node = self.end.prev()?;
		self.end = node;
		Some(Cursor {
			list: self.list,
			node: NonNull::from(node),

			phantom: PhantomData,
		})
	}
}

#[cfg(test)]
mod test {
	use super::*;

	struct Foo {
		foo: usize,
		node: ListNode,
	}

	fn init<const OFF: usize>(list: &mut List<Foo, OFF>) {
		list.insert_front(
			Arc::new(Foo {
				foo: 0,
				node: ListNode::default(),
			})
			.unwrap(),
		);
		list.insert_front(
			Arc::new(Foo {
				foo: 1,
				node: ListNode::default(),
			})
			.unwrap(),
		);
		list.insert_front(
			Arc::new(Foo {
				foo: 2,
				node: ListNode::default(),
			})
			.unwrap(),
		);
	}

	#[test]
	fn list_basic() {
		let mut list = list!(Foo, node);
		init(&mut list);

		let mut iter = list.iter();
		for (i, j) in iter.by_ref().rev().enumerate() {
			assert_eq!(i, j.value().foo);
		}
		assert!(iter.next().is_none());
	}

	#[test]
	fn list_remove() {
		let mut list = list!(Foo, node);
		init(&mut list);

		let removed = list
			.iter()
			.find(|c| c.value().foo == 1)
			.map(|mut c| c.remove());

		let mut iter = list.iter().rev();
		assert_eq!(iter.next().map(|n| n.value().foo), Some(0));
		assert_eq!(iter.next().map(|n| n.value().foo), Some(2));
		assert!(iter.next().is_none());

		assert_eq!(removed.map(|n| n.foo), Some(1));
	}

	#[test]
	fn list_lru_promote() {
		let mut list = list!(Foo, node);
		init(&mut list);

		let mut promoted = list.iter().find(|c| c.value().foo == 1).unwrap();
		promoted.lru_promote();

		let mut iter = list.iter().rev();
		assert_eq!(iter.next().map(|n| n.value().foo), Some(0));
		assert_eq!(iter.next().map(|n| n.value().foo), Some(2));
		assert_eq!(iter.next().map(|n| n.value().foo), Some(1));
		assert!(iter.next().is_none());
	}
}

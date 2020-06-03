//! A circular buffer with fixed capacity.
//! Requires Rust 1.20+
//!
//! It can be stored directly on the stack if needed.
//!
//! This queue has `O(1)` amortized inserts and removals from both ends of the
//! container. It also has `O(1)` indexing like a vector. The contained elements
//! are not required to be copyable
//!
//! This crate is inspired by [**bluss/arrayvec**](https://github.com/bluss/arrayvec)
//!
//! # Feature Flags
//! The **arraydeque** crate has the following cargo feature flags:
//!
//! - `std`
//!   - Optional, enabled by default
//!   - Conversions between `ArrayDeque` and `Vec`
//!   - Use libstd
//! 
//! - `use_generic_array`
//!   - Optional
//!   - Allow to use `GenericArray`
//!
//! # Usage
//!
//! First, add the following to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! arraydeque = "0.4"
//! ```
//!
//! Next, add this to your crate root:
//!
//! ```
//! extern crate arraydeque;
//! ```
//!
//! Currently arraydeque by default links to the standard library, but if you would
//! instead like to use arraydeque in a `#![no_std]` situation or crate you can
//! request this via:
//!
//! ```toml
//! [dependencies]
//! arraydeque = { version = "0.4", default-features = false }
//! ```
//!
//! # Behaviors
//!
//! `ArrayDeque` provides two different behaviors, `Saturating` and `Wrapping`,
//! determining whether to remove existing element automatically when pushing
//! to a full deque.
//!
//! See the [behavior module documentation](behavior/index.html) for more.

#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(has_union_feature, feature(untagged_unions))]
#![deny(missing_docs)]

#[cfg(not(any(feature = "std", test)))]
extern crate core as std;
#[cfg(feature = "use_generic_array")]
extern crate generic_array;

use std::cmp;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::marker;
use std::ops::Index;
use std::ops::IndexMut;
use std::ptr;

use array::Index as ArrayIndex;
use behavior::Behavior;
use maybe_uninit::MaybeUninit;

mod array;
pub mod behavior;
mod error;
mod maybe_uninit;
mod range;

pub use array::Array;
pub use behavior::{Saturating, Wrapping};
pub use error::CapacityError;
pub use range::RangeArgument;

/// A fixed capacity ring buffer.
///
/// It can be stored directly on the stack if needed.
///
/// The "default" usage of this type as a queue is to use `push_back` to add to
/// the queue, and `pop_front` to remove from the queue. Iterating over `ArrayDeque` goes front
/// to back.
pub struct ArrayDeque<A: Array, B: Behavior = Saturating> {
    xs: MaybeUninit<A>,
    tail: A::Index,
    len: A::Index,
    marker: marker::PhantomData<B>,
}

impl<A: Array> ArrayDeque<A, Saturating> {
    /// Add an element to the front of the deque.
    ///
    /// Return `Ok(())` if the push succeeds, or return `Err(CapacityError { *element* })`
    /// if the vector is full.
    ///
    /// # Examples
    ///
    /// ```
    /// // 1 -(+)-> [_, _, _] => [1, _, _] -> Ok(())
    /// // 2 -(+)-> [1, _, _] => [2, 1, _] -> Ok(())
    /// // 3 -(+)-> [2, 1, _] => [3, 2, 1] -> Ok(())
    /// // 4 -(+)-> [3, 2, 1] => [3, 2, 1] -> Err(CapacityError { element: 4 })
    ///
    /// use arraydeque::{ArrayDeque, CapacityError};
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.push_front(1);
    /// buf.push_front(2);
    /// buf.push_front(3);
    ///
    /// let overflow = buf.push_front(4);
    ///
    /// assert_eq!(overflow, Err(CapacityError { element: 4 }));
    /// assert_eq!(buf.back(), Some(&1));
    /// ```
    pub fn push_front(&mut self, element: A::Item) -> Result<(), CapacityError<A::Item>> {
        if !self.is_full() {
            unsafe {
                self.push_front_unchecked(element);
            }
            Ok(())
        } else {
            Err(CapacityError { element })
        }
    }

    /// Add an element to the back of the deque.
    ///
    /// Return `Ok(())` if the push succeeds, or return `Err(CapacityError { *element* })`
    /// if the vector is full.
    ///
    /// # Examples
    ///
    /// ```
    /// // [_, _, _] <-(+)- 1 => [_, _, 1] -> Ok(())
    /// // [_, _, 1] <-(+)- 2 => [_, 1, 2] -> Ok(())
    /// // [_, 1, 2] <-(+)- 3 => [1, 2, 3] -> Ok(())
    /// // [1, 2, 3] <-(+)- 4 => [1, 2, 3] -> Err(CapacityError { element: 4 })
    ///
    /// use arraydeque::{ArrayDeque, CapacityError};
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.push_back(1);
    /// buf.push_back(2);
    /// buf.push_back(3);
    ///
    /// let overflow = buf.push_back(4);
    ///
    /// assert_eq!(overflow, Err(CapacityError { element: 4 }));
    /// assert_eq!(buf.back(), Some(&3));
    /// ```
    pub fn push_back(&mut self, element: A::Item) -> Result<(), CapacityError<A::Item>> {
        if !self.is_full() {
            unsafe {
                self.push_back_unchecked(element);
            }
            Ok(())
        } else {
            Err(CapacityError { element })
        }
    }

    /// Inserts an element at `index` within the `ArrayDeque`. Whichever
    /// end is closer to the insertion point will be moved to make room,
    /// and all the affected elements will be moved to new positions.
    ///
    /// Return `Ok(())` if the push succeeds, or return `Err(CapacityError { *element* })`
    /// if the vector is full.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than `ArrayDeque`'s length
    ///
    /// # Examples
    ///
    /// ```
    /// // [_, _, _] <-(#0)- 3 => [3, _, _] -> Ok(())
    /// // [3, _, _] <-(#0)- 1 => [1, 3, _] -> Ok(())
    /// // [1, 3, _] <-(#1)- 2 => [1, 2, 3] -> Ok(())
    /// // [1, 2, 3] <-(#1)- 4 => [1, 2, 3] -> Err(CapacityError { element: 4 })
    ///
    /// use arraydeque::{ArrayDeque, CapacityError};
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.insert(0, 3);
    /// buf.insert(0, 1);
    /// buf.insert(1, 2);
    ///
    /// let overflow = buf.insert(1, 4);
    ///
    /// assert_eq!(overflow, Err(CapacityError { element: 4 }));
    /// assert_eq!(buf.back(), Some(&3));
    /// ```
    pub fn insert(&mut self, index: usize, element: A::Item) -> Result<(), CapacityError<A::Item>> {
        assert!(index <= self.len(), "index out of bounds");

        if self.is_full() {
            return Err(CapacityError { element });
        }

        unsafe {
            self.insert_unchecked(index, element);
        }

        Ok(())
    }

    /// Extend deque from front with the contents of an iterator.
    ///
    /// Does not extract more items than there is space for.
    /// No error occurs if there are more iterator elements.
    ///
    /// # Examples
    ///
    /// ```
    /// // [9, 8, 7] -(+)-> [_, _, _, _, _, _, _] => [7, 8, 9, _, _, _, _]
    /// // [6, 5, 4] -(+)-> [7, 8, 9, _, _, _, _] => [4, 5, 6, 7, 8, 9, _]
    /// // [3, 2, 1] -(+)-> [4, 5, 6, 7, 8, 9, _] => [3, 4, 5, 6, 7, 8, 9]
    ///
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 7]> = ArrayDeque::new();
    ///
    /// buf.extend_front(vec![9, 8, 7].into_iter());
    /// buf.extend_front(vec![6, 5, 4].into_iter());
    ///
    /// assert_eq!(buf.len(), 6);
    ///
    /// // max capacity reached
    /// buf.extend_front(vec![3, 2, 1].into_iter());
    ///
    /// assert_eq!(buf.len(), 7);
    /// assert_eq!(buf, vec![3, 4, 5, 6, 7, 8, 9].into());
    /// ```
    #[allow(unused_must_use)]
    pub fn extend_front<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = A::Item>,
    {
        let take = self.capacity() - self.len();
        for element in iter.into_iter().take(take) {
            self.push_front(element);
        }
    }

    /// Extend deque from back with the contents of an iterator.
    ///
    /// Does not extract more items than there is space for.
    /// No error occurs if there are more iterator elements.
    ///
    /// # Examples
    ///
    /// ```
    /// // [_, _, _, _, _, _, _] <-(+)- [1, 2, 3] => [_, _, _, _, 1, 2, 3]
    /// // [_, _, _, _, 1, 2, 3] <-(+)- [4, 5, 6] => [_, 1, 2, 3, 4, 5, 6]
    /// // [_, 1, 2, 3, 4, 5, 6] <-(+)- [7, 8, 9] => [1, 2, 3, 4, 5, 6, 7]
    ///
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 7]> = ArrayDeque::new();
    ///
    /// buf.extend_back(vec![1, 2, 3].into_iter());
    /// buf.extend_back(vec![4, 5, 6].into_iter());
    ///
    /// assert_eq!(buf.len(), 6);
    ///
    /// // max capacity reached
    /// buf.extend_back(vec![7, 8, 9].into_iter());
    ///
    /// assert_eq!(buf.len(), 7);
    /// assert_eq!(buf, vec![1, 2, 3, 4, 5, 6, 7].into());
    /// ```
    #[allow(unused_must_use)]
    pub fn extend_back<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = A::Item>,
    {
        let take = self.capacity() - self.len();
        for element in iter.into_iter().take(take) {
            self.push_back(element);
        }
    }
}

#[allow(unused_must_use)]
impl<A: Array> Extend<A::Item> for ArrayDeque<A, Saturating> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = A::Item>,
    {
        self.extend_back(iter);
    }
}

impl<A: Array> FromIterator<A::Item> for ArrayDeque<A, Saturating> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = A::Item>,
    {
        let mut array: ArrayDeque<_, Saturating> = ArrayDeque::new();
        array.extend_back(iter);
        array
    }
}

impl<A: Array> Clone for ArrayDeque<A, Saturating>
where
    A::Item: Clone,
{
    fn clone(&self) -> Self {
        self.iter().cloned().collect()
    }
}

impl<A: Array> ArrayDeque<A, Wrapping> {
    /// Add an element to the front of the deque.
    ///
    /// Return `None` if deque still has capacity, or `Some(existing)`
    /// if the deque is full, where `existing` is the backmost element being kicked out.
    ///
    /// # Examples
    ///
    /// ```
    /// // 1 -(+)-> [_, _, _] => [1, _, _] -> None
    /// // 2 -(+)-> [1, _, _] => [2, 1, _] -> None
    /// // 3 -(+)-> [2, 1, _] => [3, 2, 1] -> None
    /// // 4 -(+)-> [3, 2, 1] => [4, 3, 2] -> Some(1)
    ///
    /// use arraydeque::{ArrayDeque, Wrapping};
    ///
    /// let mut buf: ArrayDeque<[_; 3], Wrapping> = ArrayDeque::new();
    ///
    /// buf.push_front(1);
    /// buf.push_front(2);
    /// buf.push_front(3);
    ///
    /// let existing = buf.push_front(4);
    ///
    /// assert_eq!(existing, Some(1));
    /// assert_eq!(buf.back(), Some(&2));
    /// ```
    pub fn push_front(&mut self, element: A::Item) -> Option<A::Item> {
        let existing = if self.is_full() {
            if self.capacity() == 0 {
                return Some(element);
            } else {
                self.pop_back()
            }
        } else {
            None
        };

        unsafe {
            self.push_front_unchecked(element);
        }

        existing
    }

    /// Appends an element to the back of a buffer
    ///
    /// Return `None` if deque still has capacity, or `Some(existing)`
    /// if the deque is full, where `existing` is the frontmost element being kicked out.
    ///
    /// # Examples
    ///
    /// ```
    /// // [_, _, _] <-(+)- 1 => [_, _, 1] -> None
    /// // [_, _, 1] <-(+)- 2 => [_, 1, 2] -> None
    /// // [_, 1, 2] <-(+)- 3 => [1, 2, 3] -> None
    /// // [1, 2, 3] <-(+)- 4 => [2, 3, 4] -> Some(1)
    ///
    /// use arraydeque::{ArrayDeque, Wrapping};
    ///
    /// let mut buf: ArrayDeque<[_; 3], Wrapping> = ArrayDeque::new();
    ///
    /// buf.push_back(1);
    /// buf.push_back(2);
    /// buf.push_back(3);
    ///
    /// let existing = buf.push_back(4);
    ///
    /// assert_eq!(existing, Some(1));
    /// assert_eq!(buf.back(), Some(&4));
    /// ```
    pub fn push_back(&mut self, element: A::Item) -> Option<A::Item> {
        let existing = if self.is_full() {
            if self.capacity() == 0 {
                return Some(element);
            } else {
                self.pop_front()
            }
        } else {
            None
        };

        unsafe {
            self.push_back_unchecked(element);
        }

        existing
    }

    /// Extend deque from front with the contents of an iterator.
    ///
    /// Extracts all items from iterator and kicks out the backmost element if necessary.
    ///
    /// # Examples
    ///
    /// ```
    /// // [9, 8, 7] -(+)-> [_, _, _, _, _, _, _] => [7, 8, 9, _, _, _, _]
    /// // [6, 5, 4] -(+)-> [7, 8, 9, _, _, _, _] => [4, 5, 6, 7, 8, 9, _]
    /// // [3, 2, 1] -(+)-> [4, 5, 6, 7, 8, 9, _] => [1, 2, 3, 4, 5, 6, 7]
    ///
    /// use arraydeque::{ArrayDeque, Wrapping};
    ///
    /// let mut buf: ArrayDeque<[_; 7], Wrapping> = ArrayDeque::new();
    ///
    /// buf.extend_front(vec![9, 8, 7].into_iter());
    /// buf.extend_front(vec![6, 5, 4].into_iter());
    ///
    /// assert_eq!(buf.len(), 6);
    ///
    /// // max capacity reached
    /// buf.extend_front(vec![3, 2, 1].into_iter());
    ///
    /// assert_eq!(buf.len(), 7);
    /// assert_eq!(buf, vec![1, 2, 3, 4, 5, 6, 7].into());
    /// ```
    pub fn extend_front<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = A::Item>,
    {
        for element in iter.into_iter() {
            self.push_front(element);
        }
    }

    /// Extend deque from back with the contents of an iterator.
    ///
    /// Extracts all items from iterator and kicks out the frontmost element if necessary.
    ///
    /// # Examples
    ///
    /// ```
    /// // [_, _, _, _, _, _, _] <-(+)- [1, 2, 3] => [_, _, _, _, 1, 2, 3]
    /// // [_, _, _, _, 1, 2, 3] <-(+)- [4, 5, 6] => [_, 1, 2, 3, 4, 5, 6]
    /// // [_, 1, 2, 3, 4, 5, 6] <-(+)- [7, 8, 9] => [3, 4, 5, 6, 7, 8, 9]
    ///
    /// use arraydeque::{ArrayDeque, Wrapping};
    ///
    /// let mut buf: ArrayDeque<[_; 7], Wrapping> = ArrayDeque::new();
    ///
    /// buf.extend_back(vec![1, 2, 3].into_iter());
    /// buf.extend_back(vec![4, 5, 6].into_iter());
    ///
    /// assert_eq!(buf.len(), 6);
    ///
    /// // max capacity reached
    /// buf.extend_back(vec![7, 8, 9].into_iter());
    ///
    /// assert_eq!(buf.len(), 7);
    /// assert_eq!(buf, vec![3, 4, 5, 6, 7, 8, 9].into());
    /// ```
    pub fn extend_back<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = A::Item>,
    {
        for element in iter.into_iter() {
            self.push_back(element);
        }
    }
}

#[allow(unused_must_use)]
impl<A: Array> Extend<A::Item> for ArrayDeque<A, Wrapping> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = A::Item>,
    {
        let take = self.capacity() - self.len();
        for elt in iter.into_iter().take(take) {
            self.push_back(elt);
        }
    }
}

impl<A: Array> FromIterator<A::Item> for ArrayDeque<A, Wrapping> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = A::Item>,
    {
        let mut array: ArrayDeque<_, Wrapping> = ArrayDeque::new();
        array.extend_back(iter);
        array
    }
}

impl<A: Array> Clone for ArrayDeque<A, Wrapping>
where
    A::Item: Clone,
{
    fn clone(&self) -> Self {
        self.iter().cloned().collect()
    }
}

// primitive private methods
impl<A: Array, B: Behavior> ArrayDeque<A, B> {
    #[inline]
    fn wrap_add(index: usize, addend: usize) -> usize {
        wrap_add(index, addend, A::capacity())
    }

    #[inline]
    fn wrap_sub(index: usize, subtrahend: usize) -> usize {
        wrap_sub(index, subtrahend, A::capacity())
    }

    #[inline]
    fn ptr(&self) -> *const A::Item {
        self.xs.as_ptr()
    }

    #[inline]
    fn ptr_mut(&mut self) -> *mut A::Item {
        self.xs.as_mut_ptr()
    }

    #[inline]
    fn is_contiguous(&self) -> bool {
        self.tail() + self.len() < A::capacity()
    }

    #[inline]
    fn head(&self) -> usize {
        let tail = self.tail();
        let len = self.len();
        Self::wrap_add(tail, len)
    }

    #[inline]
    fn tail(&self) -> usize {
        self.tail.to_usize()
    }

    #[inline]
    unsafe fn set_tail(&mut self, tail: usize) {
        debug_assert!(tail <= self.capacity());
        self.tail = ArrayIndex::from(tail);
    }

    #[inline]
    unsafe fn set_len(&mut self, len: usize) {
        debug_assert!(len <= self.capacity());
        self.len = ArrayIndex::from(len);
    }

    #[inline]
    unsafe fn set_tail_backward(&mut self) {
        let new_tail = Self::wrap_sub(self.tail(), 1);
        let new_len = self.len() + 1;
        self.tail = ArrayIndex::from(new_tail);
        self.len = ArrayIndex::from(new_len);
    }

    #[inline]
    unsafe fn set_tail_forward(&mut self) {
        debug_assert!(self.len() > 0);

        let new_tail = Self::wrap_add(self.tail(), 1);
        let new_len = self.len() - 1;
        self.tail = ArrayIndex::from(new_tail);
        self.len = ArrayIndex::from(new_len);
    }

    #[inline]
    unsafe fn set_head_backward(&mut self) {
        debug_assert!(self.len() > 0);

        let new_len = self.len() - 1;
        self.len = ArrayIndex::from(new_len);
    }

    #[inline]
    unsafe fn set_head_forward(&mut self) {
        debug_assert!(self.len() < A::capacity());

        let new_len = self.len() + 1;
        self.len = ArrayIndex::from(new_len);
    }

    #[inline]
    unsafe fn push_front_unchecked(&mut self, element: A::Item) {
        debug_assert!(!self.is_full());

        self.set_tail_backward();
        let tail = self.tail();
        self.buffer_write(tail, element);
    }

    #[inline]
    unsafe fn push_back_unchecked(&mut self, element: A::Item) {
        debug_assert!(!self.is_full());

        let head = self.head();
        self.buffer_write(head, element);
        self.set_head_forward();
    }

    #[allow(unused_unsafe)]
    #[inline]
    unsafe fn insert_unchecked(&mut self, index: usize, element: A::Item) {
        debug_assert!(!self.is_full());

        // Move the least number of elements in the ring buffer and insert
        // the given object
        //
        // At most len/2 - 1 elements will be moved. O(min(n, n-i))
        //
        // There are three main cases:
        //  Elements are contiguous
        //      - special case when tail is 0
        //  Elements are discontiguous and the insert is in the tail section
        //  Elements are discontiguous and the insert is in the head section
        //
        // For each of those there are two more cases:
        //  Insert is closer to tail
        //  Insert is closer to head
        //
        // Key: H - self.head
        //      T - self.tail
        //      o - Valid element
        //      I - Insertion element
        //      A - The element that should be after the insertion point
        //      M - Indicates element was moved

        let idx = Self::wrap_add(self.tail(), index);

        let distance_to_tail = index;
        let distance_to_head = self.len() - index;

        let contiguous = self.is_contiguous();

        match (
            contiguous,
            distance_to_tail <= distance_to_head,
            idx >= self.tail(),
        ) {
            (true, true, _) if index == 0 => {
                // push_front
                //
                //       T
                //       I             H
                //      [A o o o o o o . . . . . . . . .]
                //
                //                       H         T
                //      [A o o o o o o o . . . . . I]
                //

                self.set_tail_backward();
            }
            (true, true, _) => {
                unsafe {
                    // contiguous, insert closer to tail:
                    //
                    //             T   I         H
                    //      [. . . o o A o o o o . . . . . .]
                    //
                    //           T               H
                    //      [. . o o I A o o o o . . . . . .]
                    //           M M
                    //
                    // contiguous, insert closer to tail and tail is 0:
                    //
                    //
                    //       T   I         H
                    //      [o o A o o o o . . . . . . . . .]
                    //
                    //                       H             T
                    //      [o I A o o o o o . . . . . . . o]
                    //       M                             M

                    let tail = self.tail();
                    let new_tail = Self::wrap_sub(self.tail(), 1);

                    self.copy(new_tail, tail, 1);
                    // Already moved the tail, so we only copy `index - 1` elements.
                    self.copy(tail, tail + 1, index - 1);

                    self.set_tail_backward();
                }
            }
            (true, false, _) => {
                unsafe {
                    //  contiguous, insert closer to head:
                    //
                    //             T       I     H
                    //      [. . . o o o o A o o . . . . . .]
                    //
                    //             T               H
                    //      [. . . o o o o I A o o . . . . .]
                    //                       M M M

                    let head = self.head();
                    self.copy(idx + 1, idx, head - idx);

                    self.set_head_forward();
                }
            }
            (false, true, true) => {
                unsafe {
                    // discontiguous, insert closer to tail, tail section:
                    //
                    //                   H         T   I
                    //      [o o o o o o . . . . . o o A o o]
                    //
                    //                   H       T
                    //      [o o o o o o . . . . o o I A o o]
                    //                           M M

                    let tail = self.tail();
                    self.copy(tail - 1, tail, index);

                    self.set_tail_backward();
                }
            }
            (false, false, true) => {
                unsafe {
                    // discontiguous, insert closer to head, tail section:
                    //
                    //           H             T         I
                    //      [o o . . . . . . . o o o o o A o]
                    //
                    //             H           T
                    //      [o o o . . . . . . o o o o o I A]
                    //       M M M                         M

                    // copy elements up to new head
                    let head = self.head();
                    self.copy(1, 0, head);

                    // copy last element into empty spot at bottom of buffer
                    self.copy(0, A::capacity() - 1, 1);

                    // move elements from idx to end forward not including ^ element
                    self.copy(idx + 1, idx, A::capacity() - 1 - idx);

                    self.set_head_forward();
                }
            }
            (false, true, false) if idx == 0 => {
                unsafe {
                    // discontiguous, insert is closer to tail, head section,
                    // and is at index zero in the internal buffer:
                    //
                    //       I                   H     T
                    //      [A o o o o o o o o o . . . o o o]
                    //
                    //                           H   T
                    //      [A o o o o o o o o o . . o o o I]
                    //                               M M M

                    // copy elements up to new tail
                    let tail = self.tail();
                    self.copy(tail - 1, tail, A::capacity() - tail);

                    // copy last element into empty spot at bottom of buffer
                    self.copy(A::capacity() - 1, 0, 1);

                    self.set_tail_backward();
                }
            }
            (false, true, false) => {
                unsafe {
                    // discontiguous, insert closer to tail, head section:
                    //
                    //             I             H     T
                    //      [o o o A o o o o o o . . . o o o]
                    //
                    //                           H   T
                    //      [o o I A o o o o o o . . o o o o]
                    //       M M                     M M M M

                    let tail = self.tail();
                    // copy elements up to new tail
                    self.copy(tail - 1, tail, A::capacity() - tail);

                    // copy last element into empty spot at bottom of buffer
                    self.copy(A::capacity() - 1, 0, 1);

                    // move elements from idx-1 to end forward not including ^ element
                    self.copy(0, 1, idx - 1);

                    self.set_tail_backward();
                }
            }
            (false, false, false) => {
                unsafe {
                    // discontiguous, insert closer to head, head section:
                    //
                    //               I     H           T
                    //      [o o o o A o o . . . . . . o o o]
                    //
                    //                     H           T
                    //      [o o o o I A o o . . . . . o o o]
                    //                 M M M

                    let head = self.head();
                    self.copy(idx + 1, idx, head - idx);

                    self.set_head_forward();
                }
            }
        }

        // tail might've been changed so we need to recalculate
        let new_idx = Self::wrap_add(self.tail(), index);
        unsafe {
            self.buffer_write(new_idx, element);
        }
    }

    /// Copies a contiguous block of memory len long from src to dst
    #[inline]
    unsafe fn copy(&mut self, dst: usize, src: usize, len: usize) {
        debug_assert!(
            dst + len <= A::capacity(),
            "cpy dst={} src={} len={} cap={}",
            dst,
            src,
            len,
            A::capacity()
        );
        debug_assert!(
            src + len <= A::capacity(),
            "cpy dst={} src={} len={} cap={}",
            dst,
            src,
            len,
            A::capacity()
        );
        ptr::copy(
            self.ptr_mut().offset(src as isize),
            self.ptr_mut().offset(dst as isize),
            len,
        );
    }

    /// Copies a potentially wrapping block of memory len long from src to dest.
    /// (abs(dst - src) + len) must be no larger than cap() (There must be at
    /// most one continuous overlapping region between src and dest).
    unsafe fn wrap_copy(&mut self, dst: usize, src: usize, len: usize) {
        #[allow(dead_code)]
        fn diff(a: usize, b: usize) -> usize {
            if a <= b {
                b - a
            } else {
                a - b
            }
        }
        debug_assert!(
            cmp::min(diff(dst, src), A::capacity() - diff(dst, src)) + len <= A::capacity(),
            "wrc dst={} src={} len={} cap={}",
            dst,
            src,
            len,
            A::capacity()
        );

        if src == dst || len == 0 {
            return;
        }

        let dst_after_src = Self::wrap_sub(dst, src) < len;

        let src_pre_wrap_len = A::capacity() - src;
        let dst_pre_wrap_len = A::capacity() - dst;
        let src_wraps = src_pre_wrap_len < len;
        let dst_wraps = dst_pre_wrap_len < len;

        match (dst_after_src, src_wraps, dst_wraps) {
            (_, false, false) => {
                // src doesn't wrap, dst doesn't wrap
                //
                //        S . . .
                // 1 [_ _ A A B B C C _]
                // 2 [_ _ A A A A B B _]
                //            D . . .
                //
                self.copy(dst, src, len);
            }
            (false, false, true) => {
                // dst before src, src doesn't wrap, dst wraps
                //
                //    S . . .
                // 1 [A A B B _ _ _ C C]
                // 2 [A A B B _ _ _ A A]
                // 3 [B B B B _ _ _ A A]
                //    . .           D .
                //
                self.copy(dst, src, dst_pre_wrap_len);
                self.copy(0, src + dst_pre_wrap_len, len - dst_pre_wrap_len);
            }
            (true, false, true) => {
                // src before dst, src doesn't wrap, dst wraps
                //
                //              S . . .
                // 1 [C C _ _ _ A A B B]
                // 2 [B B _ _ _ A A B B]
                // 3 [B B _ _ _ A A A A]
                //    . .           D .
                //
                self.copy(0, src + dst_pre_wrap_len, len - dst_pre_wrap_len);
                self.copy(dst, src, dst_pre_wrap_len);
            }
            (false, true, false) => {
                // dst before src, src wraps, dst doesn't wrap
                //
                //    . .           S .
                // 1 [C C _ _ _ A A B B]
                // 2 [C C _ _ _ B B B B]
                // 3 [C C _ _ _ B B C C]
                //              D . . .
                //
                self.copy(dst, src, src_pre_wrap_len);
                self.copy(dst + src_pre_wrap_len, 0, len - src_pre_wrap_len);
            }
            (true, true, false) => {
                // src before dst, src wraps, dst doesn't wrap
                //
                //    . .           S .
                // 1 [A A B B _ _ _ C C]
                // 2 [A A A A _ _ _ C C]
                // 3 [C C A A _ _ _ C C]
                //    D . . .
                //
                self.copy(dst + src_pre_wrap_len, 0, len - src_pre_wrap_len);
                self.copy(dst, src, src_pre_wrap_len);
            }
            (false, true, true) => {
                // dst before src, src wraps, dst wraps
                //
                //    . . .         S .
                // 1 [A B C D _ E F G H]
                // 2 [A B C D _ E G H H]
                // 3 [A B C D _ E G H A]
                // 4 [B C C D _ E G H A]
                //    . .         D . .
                //
                debug_assert!(dst_pre_wrap_len > src_pre_wrap_len);
                let delta = dst_pre_wrap_len - src_pre_wrap_len;
                self.copy(dst, src, src_pre_wrap_len);
                self.copy(dst + src_pre_wrap_len, 0, delta);
                self.copy(0, delta, len - dst_pre_wrap_len);
            }
            (true, true, true) => {
                // src before dst, src wraps, dst wraps
                //
                //    . .         S . .
                // 1 [A B C D _ E F G H]
                // 2 [A A B D _ E F G H]
                // 3 [H A B D _ E F G H]
                // 4 [H A B D _ E F F G]
                //    . . .         D .
                //
                debug_assert!(src_pre_wrap_len > dst_pre_wrap_len);
                let delta = src_pre_wrap_len - dst_pre_wrap_len;
                self.copy(delta, 0, len - src_pre_wrap_len);
                self.copy(0, A::capacity() - delta, delta);
                self.copy(dst, src, dst_pre_wrap_len);
            }
        }
    }

    #[inline]
    unsafe fn buffer_read(&mut self, offset: usize) -> A::Item {
        ptr::read(self.ptr().offset(offset as isize))
    }

    #[inline]
    unsafe fn buffer_write(&mut self, offset: usize, element: A::Item) {
        ptr::write(self.ptr_mut().offset(offset as isize), element);
    }
}

impl<A: Array, B: Behavior> ArrayDeque<A, B> {
    /// Creates an empty `ArrayDeque`.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let buf: ArrayDeque<[usize; 2]> = ArrayDeque::new();
    /// ```
    #[inline]
    pub fn new() -> ArrayDeque<A, B> {
        unsafe {
            ArrayDeque {
                xs: MaybeUninit::uninitialized(),
                tail: ArrayIndex::from(0),
                len: ArrayIndex::from(0),
                marker: marker::PhantomData,
            }
        }
    }

    /// Return the capacity of the `ArrayDeque`.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let buf: ArrayDeque<[usize; 2]> = ArrayDeque::new();
    ///
    /// assert_eq!(buf.capacity(), 2);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        A::capacity()
    }

    /// Returns the number of elements in the `ArrayDeque`.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 1]> = ArrayDeque::new();
    ///
    /// assert_eq!(buf.len(), 0);
    ///
    /// buf.push_back(1);
    ///
    /// assert_eq!(buf.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.len.to_usize()
    }

    /// Returns true if the buffer contains no elements
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 1]> = ArrayDeque::new();
    ///
    /// assert!(buf.is_empty());
    ///
    /// buf.push_back(1);
    ///
    /// assert!(!buf.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns true if the buffer is full.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 1]> = ArrayDeque::new();
    ///
    /// assert!(!buf.is_full());
    ///
    /// buf.push_back(1);
    ///
    /// assert!(buf.is_full());
    /// ```
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// Returns `true` if the `ArrayDeque` contains an element equal to the
    /// given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 2]> = ArrayDeque::new();
    ///
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.contains(&1), true);
    /// assert_eq!(buf.contains(&3), false);
    /// ```
    pub fn contains(&self, x: &A::Item) -> bool
    where
        A::Item: PartialEq<A::Item>,
    {
        let (a, b) = self.as_slices();
        a.contains(x) || b.contains(x)
    }

    /// Provides a reference to the front element, or `None` if the sequence is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 2]> = ArrayDeque::new();
    /// assert_eq!(buf.front(), None);
    ///
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.front(), Some(&1));
    /// ```
    pub fn front(&self) -> Option<&A::Item> {
        if !self.is_empty() {
            Some(&self[0])
        } else {
            None
        }
    }

    /// Provides a mutable reference to the front element, or `None` if the
    /// sequence is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 2]> = ArrayDeque::new();
    /// assert_eq!(buf.front_mut(), None);
    ///
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.front_mut(), Some(&mut 1));
    /// ```
    pub fn front_mut(&mut self) -> Option<&mut A::Item> {
        if !self.is_empty() {
            Some(&mut self[0])
        } else {
            None
        }
    }

    /// Provides a reference to the back element, or `None` if the sequence is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 2]> = ArrayDeque::new();
    ///
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.back(), Some(&2));
    /// ```
    pub fn back(&self) -> Option<&A::Item> {
        if !self.is_empty() {
            Some(&self[self.len() - 1])
        } else {
            None
        }
    }

    /// Provides a mutable reference to the back element, or `None` if the
    /// sequence is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 2]> = ArrayDeque::new();
    ///
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.back_mut(), Some(&mut 2));
    /// ```
    pub fn back_mut(&mut self) -> Option<&mut A::Item> {
        let len = self.len();
        if !self.is_empty() {
            Some(&mut self[len - 1])
        } else {
            None
        }
    }

    /// Retrieves an element in the `ArrayDeque` by index.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.get(1), Some(&1));
    /// ```
    #[inline]
    pub fn get(&self, index: usize) -> Option<&A::Item> {
        if index < self.len() {
            let idx = Self::wrap_add(self.tail(), index);
            unsafe { Some(&*self.ptr().offset(idx as isize)) }
        } else {
            None
        }
    }

    /// Retrieves an element in the `ArrayDeque` mutably by index.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.get_mut(1), Some(&mut 1));
    /// ```
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut A::Item> {
        if index < self.len() {
            let idx = Self::wrap_add(self.tail(), index);
            unsafe { Some(&mut *self.ptr_mut().offset(idx as isize)) }
        } else {
            None
        }
    }

    /// Returns a front-to-back iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// let expected = vec![0, 1, 2];
    ///
    /// assert!(buf.iter().eq(expected.iter()));
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<A::Item> {
        Iter {
            tail: self.tail(),
            len: self.len(),
            ring: self.xs.as_slice(),
        }
    }

    /// Returns a front-to-back iterator that returns mutable references.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[usize; 3]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// let mut expected = vec![0, 1, 2];
    ///
    /// assert!(buf.iter_mut().eq(expected.iter_mut()));
    /// ```
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<A::Item> {
        IterMut {
            tail: self.tail(),
            len: self.len(),
            ring: self.xs.as_mut_slice(),
        }
    }

    /// Make the buffer contiguous
    ///
    /// The linearization may be required when interacting with external
    /// interfaces requiring contiguous slices.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[usize; 10]> = ArrayDeque::new();
    /// buf.extend_back(vec![1, 2, 3]);
    /// buf.extend_front(vec![-1, -2, -3]);
    ///
    /// buf.linearize();
    ///
    /// assert_eq!(buf.as_slices().1.len(), 0);
    /// ```
    ///
    /// # Complexity
    ///
    /// Takes `O(len())` time and no extra space.
    pub fn linearize(&mut self) {
        if self.is_contiguous() {
            return;
        }

        let tail = self.tail();
        let len = self.len();
        let mut new_tail = tail;
        let mut dst : usize = 0;

        while dst < len {
            let mut src = new_tail;

            while src < A::capacity() && dst < len {
                if dst == new_tail {
                    new_tail = src;
                }

                unsafe {
                    ptr::swap(
                        self.ptr_mut().offset(dst as isize),
                        self.ptr_mut().offset(src as isize),
                    );
                }

                dst = dst + 1;
                src = src + 1;
            }
        }

        unsafe { self.set_tail(0); }
    }

    /// Removes the first element and returns it, or `None` if the sequence is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 2]> = ArrayDeque::new();
    ///
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.pop_front(), Some(1));
    /// assert_eq!(buf.pop_front(), Some(2));
    /// assert_eq!(buf.pop_front(), None);
    /// ```
    pub fn pop_front(&mut self) -> Option<A::Item> {
        if self.is_empty() {
            return None;
        }
        unsafe {
            let tail = self.tail();
            self.set_tail_forward();
            Some(self.buffer_read(tail))
        }
    }

    /// Removes the last element from a buffer and returns it, or `None` if
    /// it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 2]> = ArrayDeque::new();
    /// assert_eq!(buf.pop_back(), None);
    ///
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.pop_back(), Some(2));
    /// assert_eq!(buf.pop_back(), Some(1));
    /// ```
    pub fn pop_back(&mut self) -> Option<A::Item> {
        if self.is_empty() {
            return None;
        }
        unsafe {
            self.set_head_backward();
            let head = self.head();
            Some(self.buffer_read(head))
        }
    }

    /// Clears the buffer, removing all values.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 1]> = ArrayDeque::new();
    ///
    /// buf.push_back(1);
    /// buf.clear();
    ///
    /// assert!(buf.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.drain(..);
    }

    /// Create a draining iterator that removes the specified range in the
    /// `ArrayDeque` and yields the removed items.
    ///
    /// Note 1: The element range is removed even if the iterator is not
    /// consumed until the end.
    ///
    /// Note 2: It is unspecified how many elements are removed from the deque,
    /// if the `Drain` value is not dropped, but the borrow it holds expires
    /// (eg. due to mem::forget).
    ///
    /// # Panics
    ///
    /// Panics if the starting point is greater than the end point or if
    /// the end point is greater than the length of the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// {
    ///     let drain = buf.drain(2..);
    ///     assert!(vec![2].into_iter().eq(drain));
    /// }
    ///
    /// {
    ///     let iter = buf.iter();
    ///     assert!(vec![0, 1].iter().eq(iter));
    /// }
    ///
    /// // A full range clears all contents
    /// buf.drain(..);
    /// assert!(buf.is_empty());
    /// ```
    pub fn drain<R>(&mut self, range: R) -> Drain<A, B>
    where
        R: RangeArgument<usize>,
    {
        let len = self.len();
        let start = range.start().unwrap_or(0);
        let end = range.end().unwrap_or(len);
        assert!(start <= end, "drain lower bound was too large");
        assert!(end <= len, "drain upper bound was too large");

        let drain_tail = Self::wrap_add(self.tail(), start);
        let drain_head = Self::wrap_add(self.tail(), end);
        let drain_len = end - start;

        unsafe { self.set_len(start) }

        Drain {
            deque: self as *mut _,
            after_tail: drain_head,
            after_len: len - end,
            iter: Iter {
                tail: drain_tail,
                len: drain_len,
                ring: self.xs.as_mut_slice(),
            },
        }
    }

    /// Swaps elements at indices `i` and `j`.
    ///
    /// `i` and `j` may be equal.
    ///
    /// Fails if there is no element with either index.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// buf.swap(0, 2);
    ///
    /// assert_eq!(buf, vec![2, 1, 0].into());
    /// ```
    #[inline]
    pub fn swap(&mut self, i: usize, j: usize) {
        assert!(i < self.len());
        assert!(j < self.len());
        let ri = Self::wrap_add(self.tail(), i);
        let rj = Self::wrap_add(self.tail(), j);
        unsafe {
            ptr::swap(
                self.ptr_mut().offset(ri as isize),
                self.ptr_mut().offset(rj as isize),
            )
        }
    }

    /// Removes an element from anywhere in the `ArrayDeque` and returns it, replacing it with the
    /// last element.
    ///
    /// This does not preserve ordering, but is O(1).
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(buf.swap_remove_back(0), None);
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.swap_remove_back(0), Some(0));
    /// assert_eq!(buf, vec![2, 1].into());
    /// ```
    pub fn swap_remove_back(&mut self, index: usize) -> Option<A::Item> {
        let length = self.len();
        if length > 0 && index < length - 1 {
            self.swap(index, length - 1);
        } else if index >= length {
            return None;
        }
        self.pop_back()
    }

    /// Removes an element from anywhere in the `ArrayDeque` and returns it,
    /// replacing it with the first element.
    ///
    /// This does not preserve ordering, but is O(1).
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(buf.swap_remove_back(0), None);
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.swap_remove_front(2), Some(2));
    /// assert_eq!(buf, vec![1, 0].into());
    /// ```
    pub fn swap_remove_front(&mut self, index: usize) -> Option<A::Item> {
        let length = self.len();
        if length > 0 && index < length && index != 0 {
            self.swap(index, 0);
        } else if index >= length {
            return None;
        }
        self.pop_front()
    }

    /// Removes and returns the element at `index` from the `ArrayDeque`.
    /// Whichever end is closer to the removal point will be moved to make
    /// room, and all the affected elements will be moved to new positions.
    /// Returns `None` if `index` is out of bounds.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// assert_eq!(buf.remove(1), Some(1));
    /// assert_eq!(buf, vec![0, 2].into());
    /// ```
    pub fn remove(&mut self, index: usize) -> Option<A::Item> {
        if self.is_empty() || self.len() <= index {
            return None;
        }

        // There are three main cases:
        //  Elements are contiguous
        //  Elements are discontiguous and the removal is in the tail section
        //  Elements are discontiguous and the removal is in the head section
        //      - special case when elements are technically contiguous,
        //        but self.head = 0
        //
        // For each of those there are two more cases:
        //  Insert is closer to tail
        //  Insert is closer to head
        //
        // Key: H - self.head
        //      T - self.tail
        //      o - Valid element
        //      x - Element marked for removal
        //      R - Indicates element that is being removed
        //      M - Indicates element was moved

        let idx = Self::wrap_add(self.tail(), index);

        let elem = unsafe { Some(self.buffer_read(idx)) };

        let distance_to_tail = index;
        let distance_to_head = self.len() - index;

        let contiguous = self.is_contiguous();

        match (
            contiguous,
            distance_to_tail <= distance_to_head,
            idx >= self.tail(),
        ) {
            (true, true, _) => {
                unsafe {
                    // contiguous, remove closer to tail:
                    //
                    //             T   R         H
                    //      [. . . o o x o o o o . . . . . .]
                    //
                    //               T           H
                    //      [. . . . o o o o o o . . . . . .]
                    //               M M

                    let tail = self.tail();
                    self.copy(tail + 1, tail, index);
                    self.set_tail_forward();
                }
            }
            (true, false, _) => {
                unsafe {
                    // contiguous, remove closer to head:
                    //
                    //             T       R     H
                    //      [. . . o o o o x o o . . . . . .]
                    //
                    //             T           H
                    //      [. . . o o o o o o . . . . . . .]
                    //                     M M

                    let head = self.head();
                    self.copy(idx, idx + 1, head - idx - 1);
                    self.set_head_backward();
                }
            }
            (false, true, true) => {
                unsafe {
                    // discontiguous, remove closer to tail, tail section:
                    //
                    //                   H         T   R
                    //      [o o o o o o . . . . . o o x o o]
                    //
                    //                   H           T
                    //      [o o o o o o . . . . . . o o o o]
                    //                               M M

                    let tail = self.tail();
                    self.copy(tail + 1, tail, index);
                    self.set_tail_forward();
                }
            }
            (false, false, false) => {
                unsafe {
                    // discontiguous, remove closer to head, head section:
                    //
                    //               R     H           T
                    //      [o o o o x o o . . . . . . o o o]
                    //
                    //                   H             T
                    //      [o o o o o o . . . . . . . o o o]
                    //               M M

                    let head = self.head();
                    self.copy(idx, idx + 1, head - idx - 1);
                    self.set_head_backward();
                }
            }
            (false, false, true) => {
                unsafe {
                    // discontiguous, remove closer to head, tail section:
                    //
                    //             H           T         R
                    //      [o o o . . . . . . o o o o o x o]
                    //
                    //           H             T
                    //      [o o . . . . . . . o o o o o o o]
                    //       M M                         M M
                    //
                    // or quasi-discontiguous, remove next to head, tail section:
                    //
                    //       H                 T         R
                    //      [. . . . . . . . . o o o o o x o]
                    //
                    //                         T           H
                    //      [. . . . . . . . . o o o o o o .]
                    //                                   M

                    // draw in elements in the tail section
                    self.copy(idx, idx + 1, A::capacity() - idx - 1);

                    // Prevents underflow.
                    if self.head() != 0 {
                        // copy first element into empty spot
                        self.copy(A::capacity() - 1, 0, 1);

                        // move elements in the head section backwards
                        let head = self.head();
                        self.copy(0, 1, head - 1);
                    }

                    self.set_head_backward();
                }
            }
            (false, true, false) => {
                unsafe {
                    // discontiguous, remove closer to tail, head section:
                    //
                    //           R               H     T
                    //      [o o x o o o o o o o . . . o o o]
                    //
                    //                           H       T
                    //      [o o o o o o o o o o . . . . o o]
                    //       M M M                       M M

                    let tail = self.tail();
                    // draw in elements up to idx
                    self.copy(1, 0, idx);

                    // copy last element into empty spot
                    self.copy(0, A::capacity() - 1, 1);

                    // move elements from tail to end forward, excluding the last one
                    self.copy(tail + 1, tail, A::capacity() - tail - 1);

                    self.set_tail_forward();
                }
            }
        }

        return elem;
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated `Self`. `self` contains elements `[0, at)`,
    /// and the returned `Self` contains elements `[at, len)`.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    /// buf.push_back(2);
    ///
    /// // buf = [0], buf2 = [1, 2]
    /// let buf2 = buf.split_off(1);
    ///
    /// assert_eq!(buf.len(), 1);
    /// assert_eq!(buf2.len(), 2);
    /// ```
    #[inline]
    pub fn split_off(&mut self, at: usize) -> Self {
        let len = self.len();
        assert!(at <= len, "`at` out of bounds");

        let other_len = len - at;
        let mut other = Self::new();

        unsafe {
            let (first_half, second_half) = self.as_slices();

            let first_len = first_half.len();
            let second_len = second_half.len();
            if at < first_len {
                // `at` lies in the first half.
                let amount_in_first = first_len - at;

                ptr::copy_nonoverlapping(
                    first_half.as_ptr().offset(at as isize),
                    other.ptr_mut(),
                    amount_in_first,
                );

                // just take all of the second half.
                ptr::copy_nonoverlapping(
                    second_half.as_ptr(),
                    other.ptr_mut().offset(amount_in_first as isize),
                    second_len,
                );
            } else {
                // `at` lies in the second half, need to factor in the elements we skipped
                // in the first half.
                let offset = at - first_len;
                let amount_in_second = second_len - offset;
                ptr::copy_nonoverlapping(
                    second_half.as_ptr().offset(offset as isize),
                    other.ptr_mut(),
                    amount_in_second,
                );
            }
        }

        // Cleanup where the ends of the buffers are
        unsafe {
            self.set_len(at);
            other.set_len(other_len);
        }

        other
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&e)` returns false.
    /// This method operates in place and preserves the order of the retained
    /// elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    ///
    /// buf.extend_back(0..4);
    /// buf.retain(|&x| x % 2 == 0);
    ///
    /// assert_eq!(buf, vec![0, 2].into());
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&A::Item) -> bool,
    {
        let len = self.len();
        let mut del = 0;
        for i in 0..len {
            if !f(&self[i]) {
                del += 1;
            } else if del > 0 {
                self.swap(i - del, i);
            }
        }
        if del > 0 {
            for _ in (len - del)..self.len() {
                self.pop_back();
            }
        }
    }

    /// Returns a pair of slices which contain, in order, the contents of the
    /// `ArrayDeque`.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 7]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    ///
    /// assert_eq!(buf.as_slices(), (&[0, 1][..], &[][..]));
    ///
    /// buf.push_front(2);
    ///
    /// assert_eq!(buf.as_slices(), (&[2][..], &[0, 1][..]));
    /// ```
    #[inline]
    pub fn as_slices(&self) -> (&[A::Item], &[A::Item]) {
        unsafe {
            let (first, second) = (*(self as *const Self as *mut Self)).as_mut_slices();
            (first, second)
        }
    }

    /// Returns a pair of slices which contain, in order, the contents of the
    /// `ArrayDeque`.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 7]> = ArrayDeque::new();
    ///
    /// buf.push_back(0);
    /// buf.push_back(1);
    ///
    /// assert_eq!(buf.as_mut_slices(), (&mut [0, 1][..], &mut[][..]));
    ///
    /// buf.push_front(2);
    ///
    /// assert_eq!(buf.as_mut_slices(), (&mut[2][..], &mut[0, 1][..]));
    /// ```
    #[inline]
    pub fn as_mut_slices(&mut self) -> (&mut [A::Item], &mut [A::Item]) {
        let contiguous = self.is_contiguous();
        let head = self.head();
        let tail = self.tail();
        let buf = self.xs.as_mut_slice();

        if contiguous {
            let (empty, buf) = buf.split_at_mut(0);
            (&mut buf[tail..head], empty)
        } else {
            let (mid, right) = buf.split_at_mut(tail);
            let (left, _) = mid.split_at_mut(head);

            (right, left)
        }
    }
}

impl<A: Array> From<ArrayDeque<A, Wrapping>> for ArrayDeque<A, Saturating> {
    fn from(buf: ArrayDeque<A, Wrapping>) -> Self {
        buf.into_iter().collect()
    }
}

impl<A: Array> From<ArrayDeque<A, Saturating>> for ArrayDeque<A, Wrapping> {
    fn from(buf: ArrayDeque<A, Saturating>) -> Self {
        buf.into_iter().collect()
    }
}

#[cfg(feature = "std")]
impl<A: Array, B: Behavior> From<Vec<A::Item>> for ArrayDeque<A, B>
where
    Self: FromIterator<A::Item>,
{
    fn from(vec: Vec<A::Item>) -> Self {
        vec.into_iter().collect()
    }
}

#[cfg(feature = "std")]
impl<A: Array, B: Behavior> Into<Vec<A::Item>> for ArrayDeque<A, B>
where
    Self: FromIterator<A::Item>,
{
    fn into(self) -> Vec<A::Item> {
        self.into_iter().collect()
    }
}

impl<A: Array, B: Behavior> Drop for ArrayDeque<A, B> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<A: Array, B: Behavior> Default for ArrayDeque<A, B> {
    #[inline]
    fn default() -> Self {
        ArrayDeque::new()
    }
}

impl<A: Array, B: Behavior> PartialEq for ArrayDeque<A, B>
where
    A::Item: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let (sa, sb) = self.as_slices();
        let (oa, ob) = other.as_slices();
        if sa.len() == oa.len() {
            sa == oa && sb == ob
        } else if sa.len() < oa.len() {
            // Always divisible in three sections, for example:
            // self:  [a b c|d e f]
            // other: [0 1 2 3|4 5]
            // front = 3, mid = 1,
            // [a b c] == [0 1 2] && [d] == [3] && [e f] == [4 5]
            let front = sa.len();
            let mid = oa.len() - front;

            let (oa_front, oa_mid) = oa.split_at(front);
            let (sb_mid, sb_back) = sb.split_at(mid);
            debug_assert_eq!(sa.len(), oa_front.len());
            debug_assert_eq!(sb_mid.len(), oa_mid.len());
            debug_assert_eq!(sb_back.len(), ob.len());
            sa == oa_front && sb_mid == oa_mid && sb_back == ob
        } else {
            let front = oa.len();
            let mid = sa.len() - front;

            let (sa_front, sa_mid) = sa.split_at(front);
            let (ob_mid, ob_back) = ob.split_at(mid);
            debug_assert_eq!(sa_front.len(), oa.len());
            debug_assert_eq!(sa_mid.len(), ob_mid.len());
            debug_assert_eq!(sb.len(), ob_back.len());
            sa_front == oa && sa_mid == ob_mid && sb == ob_back
        }
    }
}

impl<A: Array, B: Behavior> Eq for ArrayDeque<A, B> where A::Item: Eq {}

impl<A: Array, B: Behavior> PartialOrd for ArrayDeque<A, B>
where
    A::Item: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<A: Array, B: Behavior> Ord for ArrayDeque<A, B>
where
    A::Item: Ord,
{
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<A: Array, B: Behavior> Hash for ArrayDeque<A, B>
where
    A::Item: Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        let (a, b) = self.as_slices();
        Hash::hash_slice(a, state);
        Hash::hash_slice(b, state);
    }
}

impl<A: Array, B: Behavior> Index<usize> for ArrayDeque<A, B> {
    type Output = A::Item;

    #[inline]
    fn index(&self, index: usize) -> &A::Item {
        let len = self.len();
        self.get(index)
            .or_else(|| {
                panic!(
                    "index out of bounds: the len is {} but the index is {}",
                    len, index
                )
            }).unwrap()
    }
}

impl<A: Array, B: Behavior> IndexMut<usize> for ArrayDeque<A, B> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut A::Item {
        let len = self.len();
        self.get_mut(index)
            .or_else(|| {
                panic!(
                    "index out of bounds: the len is {} but the index is {}",
                    len, index
                )
            }).unwrap()
    }
}

impl<A: Array, B: Behavior> IntoIterator for ArrayDeque<A, B> {
    type Item = A::Item;
    type IntoIter = IntoIter<A, B>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { inner: self }
    }
}

impl<'a, A: Array, B: Behavior> IntoIterator for &'a ArrayDeque<A, B> {
    type Item = &'a A::Item;
    type IntoIter = Iter<'a, A::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, A: Array, B: Behavior> IntoIterator for &'a mut ArrayDeque<A, B> {
    type Item = &'a mut A::Item;
    type IntoIter = IterMut<'a, A::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<A: Array, B: Behavior> fmt::Debug for ArrayDeque<A, B>
where
    A::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

#[inline]
fn wrap_add(index: usize, addend: usize, capacity: usize) -> usize {
    debug_assert!(addend <= capacity);
    (index + addend) % capacity
}

#[inline]
fn wrap_sub(index: usize, subtrahend: usize, capacity: usize) -> usize {
    debug_assert!(subtrahend <= capacity);
    (index + capacity - subtrahend) % capacity
}

/// `ArrayDeque` iterator
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct Iter<'a, T: 'a> {
    ring: &'a [T],
    tail: usize,
    len: usize,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        if self.len == 0 {
            return None;
        }
        let tail = self.tail;
        self.tail = wrap_add(self.tail, 1, self.ring.len());
        self.len -= 1;
        unsafe { Some(self.ring.get_unchecked(tail)) }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        let head = wrap_add(self.tail, self.len, self.ring.len());
        unsafe { Some(self.ring.get_unchecked(head)) }
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {}

/// `ArrayDeque` mutable iterator
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct IterMut<'a, T: 'a> {
    ring: &'a mut [T],
    tail: usize,
    len: usize,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<&'a mut T> {
        if self.len == 0 {
            return None;
        }
        let tail = self.tail;
        self.tail = wrap_add(self.tail, 1, self.ring.len());
        self.len -= 1;
        unsafe {
            let elem = self.ring.get_unchecked_mut(tail);
            Some(&mut *(elem as *mut _))
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        let head = wrap_add(self.tail, self.len, self.ring.len());
        unsafe {
            let elem = self.ring.get_unchecked_mut(head);
            Some(&mut *(elem as *mut _))
        }
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {}

/// By-value `ArrayDeque` iterator
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct IntoIter<A: Array, B: Behavior> {
    inner: ArrayDeque<A, B>,
}

impl<A: Array, B: Behavior> Iterator for IntoIter<A, B> {
    type Item = A::Item;

    #[inline]
    fn next(&mut self) -> Option<A::Item> {
        self.inner.pop_front()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.inner.len();
        (len, Some(len))
    }
}

impl<A: Array, B: Behavior> DoubleEndedIterator for IntoIter<A, B> {
    #[inline]
    fn next_back(&mut self) -> Option<A::Item> {
        self.inner.pop_back()
    }
}

impl<A: Array, B: Behavior> ExactSizeIterator for IntoIter<A, B> {}

/// Draining `ArrayDeque` iterator
pub struct Drain<'a, A, B>
where
    A: Array,
    A::Item: 'a,
    B: Behavior,
{
    after_tail: usize,
    after_len: usize,
    iter: Iter<'a, A::Item>,
    deque: *mut ArrayDeque<A, B>,
}

impl<'a, A, B> Drop for Drain<'a, A, B>
where
    A: Array,
    A::Item: 'a,
    B: Behavior,
{
    fn drop(&mut self) {
        for _ in self.by_ref() {}

        let source_deque = unsafe { &mut *self.deque };

        let tail_len = source_deque.len();
        let head_len = self.after_len;

        let orig_tail = source_deque.tail();
        let drain_tail = wrap_add(orig_tail, tail_len, A::capacity());
        let drain_head = self.after_tail;
        let orig_head = wrap_add(drain_head, head_len, A::capacity());
        let orig_len = wrap_sub(orig_head, orig_tail, A::capacity());

        // Restore the original len value
        unsafe { source_deque.set_len(orig_len) }
        match (tail_len, head_len) {
            (0, 0) => unsafe {
                source_deque.set_tail(0);
                source_deque.set_len(0);
            },
            (0, _) => unsafe {
                source_deque.set_tail(drain_head);
                source_deque.set_len(head_len);
            },
            (_, 0) => unsafe { source_deque.set_len(tail_len) },
            _ => unsafe {
                if tail_len <= head_len {
                    let new_tail = wrap_sub(drain_head, tail_len, A::capacity());
                    source_deque.set_tail(new_tail);
                    source_deque.set_len(tail_len + head_len);
                    source_deque.wrap_copy(new_tail, orig_tail, tail_len);
                } else {
                    source_deque.set_len(tail_len + head_len);
                    source_deque.wrap_copy(drain_tail, drain_head, head_len);
                }
            },
        }
    }
}

impl<'a, A, B> Iterator for Drain<'a, A, B>
where
    A: Array,
    A::Item: 'a,
    B: Behavior,
{
    type Item = A::Item;

    #[inline]
    fn next(&mut self) -> Option<A::Item> {
        self.iter.next().map(|elt| unsafe { ptr::read(elt) })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, A, B> DoubleEndedIterator for Drain<'a, A, B>
where
    A: Array,
    A::Item: 'a,
    B: Behavior,
{
    #[inline]
    fn next_back(&mut self) -> Option<A::Item> {
        self.iter.next_back().map(|elt| unsafe { ptr::read(elt) })
    }
}

impl<'a, A, B> ExactSizeIterator for Drain<'a, A, B>
where
    A: Array,
    A::Item: 'a,
    B: Behavior,
{}

#[cfg(test)]
mod tests {
    #![allow(unused_must_use)]
    use super::*;

    #[test]
    fn test_simple() {
        macro_rules! test {
            ($behavior:ident) => {{
                let mut tester: ArrayDeque<[_; 7], $behavior> = ArrayDeque::new();
                assert_eq!(tester.capacity(), 7);
                assert_eq!(tester.len(), 0);

                tester.push_back(1);
                tester.push_back(2);
                tester.push_back(3);
                tester.push_back(4);
                assert_eq!(tester.len(), 4);
                assert_eq!(tester.pop_front(), Some(1));
                assert_eq!(tester.pop_front(), Some(2));
                assert_eq!(tester.len(), 2);
                assert_eq!(tester.pop_front(), Some(3));
                assert_eq!(tester.pop_front(), Some(4));
                assert_eq!(tester.pop_front(), None);
            }};
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn test_simple_reversely() {
        macro_rules! test {
            ($behavior:ident) => {{
                let mut tester: ArrayDeque<[_; 7], $behavior> = ArrayDeque::new();
                assert_eq!(tester.capacity(), 7);
                assert_eq!(tester.len(), 0);

                tester.push_front(1);
                tester.push_front(2);
                tester.push_front(3);
                tester.push_front(4);
                assert_eq!(tester.len(), 4);
                assert_eq!(tester.pop_back(), Some(1));
                assert_eq!(tester.pop_back(), Some(2));
                assert_eq!(tester.len(), 2);
                assert_eq!(tester.pop_back(), Some(3));
                assert_eq!(tester.pop_back(), Some(4));
                assert_eq!(tester.pop_back(), None);
            }};
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn test_overflow_saturating() {
        let mut tester: ArrayDeque<[_; 2], Saturating> = ArrayDeque::new();
        assert_eq!(tester.push_back(1), Ok(()));
        assert_eq!(tester.push_back(2), Ok(()));
        assert_eq!(tester.push_back(3), Err(CapacityError { element: 3 }));

        let mut tester: ArrayDeque<[_; 2], Saturating> = ArrayDeque::new();
        assert_eq!(tester.insert(0, 1), Ok(()));
        assert_eq!(tester.insert(1, 2), Ok(()));
        assert_eq!(tester.insert(2, 3), Err(CapacityError { element: 3 }));
    }

    #[test]
    fn test_overflow_wrapping() {
        let mut tester: ArrayDeque<[_; 2], Wrapping> = ArrayDeque::new();
        assert_eq!(tester.push_back(1), None);
        assert_eq!(tester.push_back(2), None);
        assert_eq!(tester.push_back(3), Some(1));
    }

    #[test]
    fn test_pop_empty() {
        let mut tester: ArrayDeque<[_; 2]> = ArrayDeque::new();
        assert_eq!(tester.push_back(1), Ok(()));
        assert_eq!(tester.pop_front(), Some(1));
        assert_eq!(tester.is_empty(), true);
        assert_eq!(tester.len(), 0);
        assert_eq!(tester.pop_front(), None);
    }

    #[test]
    fn test_index() {
        let mut tester: ArrayDeque<[_; 3]> = ArrayDeque::new();
        tester.push_back(1);
        tester.push_back(2);
        tester.push_back(3);
        assert_eq!(tester[0], 1);
        // pop_front 1 <- [2, 3]
        assert_eq!(tester.pop_front(), Some(1));
        assert_eq!(tester[0], 2);
        assert_eq!(tester.len(), 2);
        // push_front 0 -> [0, 2, 3]
        tester.push_front(0);
        assert_eq!(tester[0], 0);
        // [0, 2] -> 3 pop_back
        assert_eq!(tester.pop_back(), Some(3));
        assert_eq!(tester[1], 2);
    }

    #[test]
    #[should_panic]
    fn test_index_overflow() {
        let mut tester: ArrayDeque<[_; 3]> = ArrayDeque::new();
        tester.push_back(1);
        tester.push_back(2);
        tester[2];
    }

    #[test]
    fn test_iter() {
        let mut tester: ArrayDeque<[_; 2]> = ArrayDeque::new();
        tester.push_back(1);
        tester.push_back(2);
        {
            let mut iter = tester.iter();
            assert_eq!(iter.size_hint(), (2, Some(2)));
            assert_eq!(iter.next(), Some(&1));
            assert_eq!(iter.next(), Some(&2));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.size_hint(), (0, Some(0)));
        }
        tester.pop_front();
        tester.push_back(3);
        {
            let mut iter = (&tester).into_iter();
            assert_eq!(iter.next(), Some(&2));

            // test clone
            let mut iter2 = iter.clone();
            assert_eq!(iter.next(), Some(&3));
            assert_eq!(iter.next(), None);
            assert_eq!(iter2.next(), Some(&3));
            assert_eq!(iter2.next(), None);
        }
    }

    #[test]
    fn test_iter_mut() {
        let mut tester: ArrayDeque<[_; 2]> = ArrayDeque::new();
        tester.push_back(1);
        tester.push_back(2);
        {
            let mut iter = tester.iter_mut();
            assert_eq!(iter.size_hint(), (2, Some(2)));
            assert_eq!(iter.next(), Some(&mut 1));
            assert_eq!(iter.next(), Some(&mut 2));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.size_hint(), (0, Some(0)));
        }
        tester.pop_front();
        tester.push_back(3);
        {
            let mut iter = (&mut tester).into_iter();
            assert_eq!(iter.next(), Some(&mut 2));
            assert_eq!(iter.next(), Some(&mut 3));
            assert_eq!(iter.next(), None);
        }
        {
            // mutation
            let mut iter = tester.iter_mut();
            iter.next().map(|n| *n += 1);
            iter.next().map(|n| *n += 2);
        }
        assert_eq!(tester[0], 3);
        assert_eq!(tester[1], 5);
    }

    #[test]
    fn test_into_iter() {
        #[derive(Eq, PartialEq, Debug)]
        struct NoCopy<T>(T);

        {
            let mut tester: ArrayDeque<[NoCopy<u8>; 3]> = ArrayDeque::new();
            tester.push_back(NoCopy(1));
            tester.push_back(NoCopy(2));
            let mut iter = tester.into_iter();
            assert_eq!(iter.size_hint(), (2, Some(2)));
            assert_eq!(iter.next(), Some(NoCopy(1)));
            assert_eq!(iter.next(), Some(NoCopy(2)));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.size_hint(), (0, Some(0)));
        }
        {
            let mut tester: ArrayDeque<[NoCopy<u8>; 3]> = ArrayDeque::new();
            tester.push_back(NoCopy(1));
            tester.push_back(NoCopy(2));
            tester.pop_front();
            tester.push_back(NoCopy(3));
            let mut iter = tester.into_iter();
            assert_eq!(iter.next(), Some(NoCopy(2)));
            assert_eq!(iter.next(), Some(NoCopy(3)));
            assert_eq!(iter.next(), None);
        }
        {
            let mut tester: ArrayDeque<[NoCopy<u8>; 3]> = ArrayDeque::new();
            tester.push_back(NoCopy(1));
            tester.push_back(NoCopy(2));
            tester.pop_front();
            tester.push_back(NoCopy(3));
            tester.pop_front();
            tester.push_back(NoCopy(4));
            let mut iter = tester.into_iter();
            assert_eq!(iter.next(), Some(NoCopy(3)));
            assert_eq!(iter.next(), Some(NoCopy(4)));
            assert_eq!(iter.next(), None);
        }
    }

    #[test]
    fn test_drain() {
        const CAP: usize = 8;
        let mut tester: ArrayDeque<[_; CAP]> = ArrayDeque::new();

        for padding in 0..CAP {
            for drain_start in 0..CAP {
                for drain_end in drain_start..CAP {
                    // deque starts from different tail position
                    unsafe {
                        tester.set_len(0);
                        tester.set_tail(padding);
                    }

                    tester.extend_back(0..CAP);

                    let mut expected = vec![0, 1, 2, 3, 4, 5, 6, 7];
                    let drains: Vec<_> = tester.drain(drain_start..drain_end).collect();
                    let expected_drains: Vec<_> = expected.drain(drain_start..drain_end).collect();
                    assert_eq!(drains, expected_drains);
                    assert_eq!(tester, expected.into());
                }
            }
        }
    }

    #[test]
    fn test_drop() {
        use std::cell::Cell;

        let flag = &Cell::new(0);

        struct Bump<'a>(&'a Cell<i32>);

        impl<'a> Drop for Bump<'a> {
            fn drop(&mut self) {
                let n = self.0.get();
                self.0.set(n + 1);
            }
        }

        {
            let mut tester = ArrayDeque::<[Bump; 128]>::new();
            tester.push_back(Bump(flag));
            tester.push_back(Bump(flag));
        }
        assert_eq!(flag.get(), 2);

        // test something with the nullable pointer optimization
        flag.set(0);
        {
            let mut tester = ArrayDeque::<[_; 3]>::new();
            tester.push_back(vec![Bump(flag)]);
            tester.push_back(vec![Bump(flag), Bump(flag)]);
            tester.push_back(vec![]);
            tester.push_back(vec![Bump(flag)]);
            assert_eq!(flag.get(), 1);
            drop(tester.pop_back());
            assert_eq!(flag.get(), 1);
            drop(tester.pop_back());
            assert_eq!(flag.get(), 3);
        }
        assert_eq!(flag.get(), 4);
    }

    #[test]
    fn test_as_slice() {
        const CAP: usize = 10;
        let mut tester = ArrayDeque::<[_; CAP]>::new();

        for len in 0..CAP + 1 {
            for padding in 0..CAP {
                // deque starts from different tail position
                unsafe {
                    tester.set_len(0);
                    tester.set_tail(padding);
                }

                let mut expected = vec![];
                tester.extend_back(0..len);
                expected.extend(0..len);

                let split_idx = CAP - padding;
                if split_idx < len {
                    assert_eq!(tester.as_slices(), expected[..].split_at(split_idx));
                } else {
                    assert_eq!(tester.as_slices(), (&expected[..], &[][..]));
                }
            }
        }
    }

    #[test]
    fn test_partial_equal() {
        const CAP: usize = 10;
        let mut tester = ArrayDeque::<[f64; CAP]>::new();

        for len in 0..CAP + 1 {
            for padding in 0..CAP {
                // deque starts from different tail position
                unsafe {
                    tester.set_len(0);
                    tester.set_tail(padding);
                }

                let mut expected = ArrayDeque::<[f64; CAP]>::new();
                for x in 0..len {
                    tester.push_back(x as f64);
                    expected.push_back(x as f64);
                }
                assert_eq!(tester, expected);

                // test negative
                if len > 2 {
                    tester.pop_front();
                    expected.pop_back();
                    assert!(tester != expected);
                }
            }
        }
    }

    #[test]
    fn test_fmt() {
        let mut tester = ArrayDeque::<[_; 5]>::new();
        tester.extend_back(0..4);
        assert_eq!(format!("{:?}", tester), "[0, 1, 2, 3]");
    }

    #[test]
    fn test_swap_front_back_remove() {
        fn test(back: bool) {
            const CAP: usize = 16;
            let mut tester = ArrayDeque::<[_; CAP]>::new();
            let usable_cap = tester.capacity();
            let final_len = usable_cap / 2;

            for len in 0..final_len {
                let expected = if back {
                    (0..len).collect()
                } else {
                    (0..len).rev().collect()
                };
                for padding in 0..usable_cap {
                    unsafe {
                        tester.set_tail(padding);
                        tester.set_len(0);
                    }
                    if back {
                        for i in 0..len * 2 {
                            tester.push_front(i);
                        }
                        for i in 0..len {
                            assert_eq!(tester.swap_remove_back(i), Some(len * 2 - 1 - i));
                        }
                    } else {
                        for i in 0..len * 2 {
                            tester.push_back(i);
                        }
                        for i in 0..len {
                            let idx = tester.len() - 1 - i;
                            assert_eq!(tester.swap_remove_front(idx), Some(len * 2 - 1 - i));
                        }
                    }
                    assert!(tester.tail() < CAP);
                    assert!(tester.head() < CAP);
                    assert_eq!(tester, expected);
                }
            }
        }
        test(true);
        test(false);
    }

    #[test]
    fn test_retain() {
        const CAP: usize = 10;
        let mut tester: ArrayDeque<[_; CAP]> = ArrayDeque::new();
        for padding in 0..CAP {
            unsafe {
                tester.set_tail(padding);
                tester.set_len(0);
            }
            tester.extend_back(0..CAP);
            tester.retain(|x| x % 2 == 0);
            assert_eq!(tester.iter().count(), CAP / 2);
        }
    }

    #[test]
    fn test_split_off() {
        const CAP: usize = 16;
        let mut tester = ArrayDeque::<[_; CAP]>::new();
        for len in 0..CAP + 1 {
            // index to split at
            for at in 0..len + 1 {
                for padding in 0..CAP {
                    let expected_self = (0..).take(at).collect();
                    let expected_other = (at..).take(len - at).collect();
                    unsafe {
                        tester.set_len(0);
                        tester.set_tail(padding);
                    }
                    for i in 0..len {
                        tester.push_back(i);
                    }
                    let result = tester.split_off(at);
                    assert!(tester.tail() < CAP);
                    assert!(tester.head() < CAP);
                    assert!(result.tail() < CAP);
                    assert!(result.head() < CAP);
                    assert_eq!(tester, expected_self);
                    assert_eq!(result, expected_other);
                }
            }
        }
    }

    #[test]
    fn test_remove() {
        const CAP: usize = 16;
        let mut tester = ArrayDeque::<[_; CAP]>::new();

        // len is the length *after* removal
        for len in 0..CAP {
            // 0, 1, 2, .., len - 1
            let expected = (0..).take(len).collect();
            for padding in 0..CAP {
                for to_remove in 0..len + 1 {
                    unsafe {
                        tester.set_tail(padding);
                        tester.set_len(0);
                    }
                    for i in 0..len {
                        if i == to_remove {
                            tester.push_back(1234);
                        }
                        tester.push_back(i);
                    }
                    if to_remove == len {
                        tester.push_back(1234);
                    }
                    tester.remove(to_remove);
                    assert!(tester.tail() < CAP);
                    assert!(tester.head() < CAP);
                    assert_eq!(tester, expected);
                }
            }
        }
    }

    #[test]
    fn test_clone() {
        let tester: ArrayDeque<[_; 16]> = (0..16).into_iter().collect();
        let cloned = tester.clone();
        assert_eq!(tester, cloned)
    }

    #[test]
    fn test_option_encoding() {
        let tester: ArrayDeque<[Box<()>; 100]> = ArrayDeque::new();
        assert!(Some(tester).is_some());
    }

    #[test]
    fn test_insert_unchecked() {
        const CAP: usize = 16;
        let mut tester = ArrayDeque::<[_; CAP]>::new();

        // len is the length *after* insertion
        for len in 1..CAP {
            // 0, 1, 2, .., len - 1
            let expected = (0..).take(len).collect();
            for padding in 0..CAP {
                for to_insert in 0..len {
                    unsafe {
                        tester.set_tail(padding);
                        tester.set_len(0);
                    }
                    for i in 0..len {
                        if i != to_insert {
                            tester.push_back(i);
                        }
                    }
                    unsafe { tester.insert_unchecked(to_insert, to_insert) };
                    assert!(tester.tail() < CAP);
                    assert!(tester.head() < CAP);
                    assert_eq!(tester, expected);
                }
            }
        }
    }

    #[test]
    fn test_linearize() {
        let mut tester: ArrayDeque<[isize; 10], Saturating> = ArrayDeque::new();
        tester.extend_back(vec![1, 2, 3]);
        tester.extend_front(vec![-1, -2]);
        assert_eq!(tester, vec![-2, -1, 1, 2, 3].into());
        tester.linearize();
        assert_eq!(tester, vec![-2, -1, 1, 2, 3].into());
        assert_eq!(tester.as_slices().1.len(), 0);

        let mut tester: ArrayDeque<[isize; 10], Saturating> = ArrayDeque::new();
        tester.extend_back(vec![1, 2]);
        tester.extend_front(vec![-1, -2, -3]);
        assert_eq!(tester, vec![-3, -2, -1, 1, 2].into());
        tester.linearize();
        assert_eq!(tester, vec![-3, -2, -1, 1, 2].into());
        assert_eq!(tester.as_slices().1.len(), 0);

        let mut tester: ArrayDeque<[isize; 10], Saturating> = ArrayDeque::new();
        tester.extend_back(vec![1, 2, 3]);
        tester.extend_front(vec![-1, -2, -3]);
        assert_eq!(tester, vec![-3, -2, -1, 1, 2, 3].into());
        tester.linearize();
        assert_eq!(tester, vec![-3, -2, -1, 1, 2, 3].into());
        assert_eq!(tester.as_slices().1.len(), 0);

        let mut tester: ArrayDeque<[isize; 5], Saturating> = ArrayDeque::new();
        tester.extend_back(vec![1, 2, 3]);
        tester.extend_front(vec![-1, -2]);
        assert_eq!(tester, vec![-2, -1, 1, 2, 3].into());
        tester.linearize();
        assert_eq!(tester, vec![-2, -1, 1, 2, 3].into());
        assert_eq!(tester.as_slices().1.len(), 0);

        let mut tester: ArrayDeque<[isize; 5], Saturating> = ArrayDeque::new();
        tester.extend_back(vec![1, 2]);
        tester.extend_front(vec![-1, -2, -3]);
        assert_eq!(tester, vec![-3, -2, -1, 1, 2].into());
        tester.linearize();
        assert_eq!(tester, vec![-3, -2, -1, 1, 2].into());
        assert_eq!(tester.as_slices().1.len(), 0);

        let mut tester: ArrayDeque<[isize; 6], Saturating> = ArrayDeque::new();
        tester.extend_back(vec![1, 2, 3]);
        tester.extend_front(vec![-1, -2, -3]);
        assert_eq!(tester, vec![-3, -2, -1, 1, 2, 3].into());
        tester.linearize();
        assert_eq!(tester, vec![-3, -2, -1, 1, 2, 3].into());
        assert_eq!(tester.as_slices().1.len(), 0);

        let mut tester: ArrayDeque<[isize; 10], Saturating> = ArrayDeque::new();
        tester.extend_back(vec![1, 2, 3, 4, 5]);
        tester.extend_front(vec![-1]);
        assert_eq!(tester, vec![-1, 1, 2, 3, 4, 5].into());
        tester.linearize();
        assert_eq!(tester, vec![-1, 1, 2, 3, 4, 5].into());
        assert_eq!(tester.as_slices().1.len(), 0);

        let mut tester: ArrayDeque<[isize; 10], Saturating> = ArrayDeque::new();
        tester.extend_back(vec![1]);
        tester.extend_front(vec![-1, -2, -3, -4, -5]);
        assert_eq!(tester, vec![-5, -4, -3, -2, -1, 1].into());
        tester.linearize();
        assert_eq!(tester, vec![-5, -4, -3, -2, -1, 1].into());
        assert_eq!(tester.as_slices().1.len(), 0);
    }

    #[test]
    fn test_from_iterator_saturating() {
        assert_eq!(
            ArrayDeque::<[_; 3], Saturating>::from_iter(vec![1, 2, 3]),
            vec![1, 2, 3].into()
        );
        assert_eq!(
            ArrayDeque::<[_; 3], Saturating>::from_iter(vec![1, 2, 3, 4, 5]),
            vec![1, 2, 3].into()
        );
    }

    #[test]
    fn test_from_iterator_wrapping() {
        assert_eq!(
            ArrayDeque::<[_; 3], Wrapping>::from_iter(vec![1, 2, 3]),
            vec![1, 2, 3].into()
        );
        assert_eq!(
            ArrayDeque::<[_; 3], Wrapping>::from_iter(vec![1, 2, 3, 4, 5]),
            vec![3, 4, 5].into()
        );
    }

    #[test]
    fn test_extend_front_saturating() {
        let mut tester: ArrayDeque<[usize; 3], Saturating> = ArrayDeque::new();
        tester.extend_front(vec![1, 2, 3]);
        assert_eq!(tester, vec![3, 2, 1].into());
        tester.extend_front(vec![4, 5]);
        assert_eq!(tester, vec![3, 2, 1].into());
    }

    #[test]
    fn test_extend_back_saturating() {
        let mut tester: ArrayDeque<[usize; 3], Saturating> = ArrayDeque::new();
        tester.extend_back(vec![1, 2, 3]);
        assert_eq!(tester, vec![1, 2, 3].into());
        tester.extend_back(vec![4, 5]);
        assert_eq!(tester, vec![1, 2, 3].into());
    }

    #[test]
    fn test_extend_front_wrapping() {
        let mut tester: ArrayDeque<[usize; 3], Wrapping> = ArrayDeque::new();
        tester.extend_front(vec![1, 2, 3]);
        assert_eq!(tester, vec![3, 2, 1].into());
        tester.extend_front(vec![4, 5]);
        assert_eq!(tester, vec![5, 4, 3].into());
    }

    #[test]
    fn test_extend_back_wrapping() {
        let mut tester: ArrayDeque<[usize; 3], Wrapping> = ArrayDeque::new();
        tester.extend_back(vec![1, 2, 3]);
        assert_eq!(tester, vec![1, 2, 3].into());
        tester.extend_back(vec![4, 5]);
        assert_eq!(tester, vec![3, 4, 5].into());
    }
}

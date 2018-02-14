//! A circular buffer with fixed capacity.
//! Requires Rust 1.15+
//!
//! It can be stored directly on the stack if needed.
//!
//! This queue has `O(1)` amortized inserts and removals from both ends of the
//! container. It also has `O(1)` indexing like a vector. The contained elements
//! are not required to be copyable
//!
//! This crate is inspired by [**bluss/arrayvec**]
//! [**bluss/arrayvec**]: https://github.com/bluss/arrayvec
//!
//! # Feature Flags
//! The **arraydeque** crate has the following cargo feature flags:
//!
//! - `std`
//!   - Optional, enabled by default
//!   - Use libstd
//!
//!
//! - `use_union`
//!   - Optional
//!   - Requires Rust nightly channel
//!   - Use the unstable feature untagged unions for the internal implementation,
//!     which has reduced space overhead
//!
//!
//! - `use_generic_array`
//!   - Optional
//!   - Requires Rust stable channel
//!   - Depend on generic-array and allow using it just like a fixed
//!     size array for ArrayDeque storage.
//!
//!
//! # Usage
//!
//! First, add the following to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! arraydeque = "0.2"
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
//! arraydeque = { version = "0.3", default-features = false }
//! ```
//!
//! # Capacity
//!
//! Note that the `capacity()` is always `backed_array.len() - 1`.
//! [Read more]
//!
//! [Read more]: https://en.wikipedia.org/wiki/Circular_buffer
//!
//! # Examples
//! ```
//! extern crate arraydeque;
//!
//! use arraydeque::ArrayDeque;
//!
//! fn main() {
//!     let mut vector: ArrayDeque<[_; 8]> = ArrayDeque::new();
//!     assert_eq!(vector.capacity(), 7);
//!     assert_eq!(vector.len(), 0);
//!
//!     vector.push_back(1);
//!     vector.push_back(2);
//!     assert_eq!(vector.len(), 2);
//!
//!     assert_eq!(vector.pop_front(), Some(1));
//!     assert_eq!(vector.pop_front(), Some(2));
//!     assert_eq!(vector.pop_front(), None);
//! }
//! ```
//!
//! # Insert & Remove
//! ```
//! use arraydeque::ArrayDeque;
//!
//! let mut vector: ArrayDeque<[_; 8]> = ArrayDeque::new();
//!
//! vector.push_back(11);
//! vector.push_back(13);
//! vector.insert(1, 12);
//! vector.remove(0);
//!
//! assert_eq!(vector[0], 12);
//! assert_eq!(vector[1], 13);
//! ```
//!
//! # Append & Extend
//! ```
//! use arraydeque::ArrayDeque;
//!
//! let mut vector: ArrayDeque<[_; 8]> = ArrayDeque::new();
//! let mut vector2: ArrayDeque<[_; 8]> = ArrayDeque::new();
//!
//! vector.extend(0..5);
//! vector2.extend(5..7);
//!
//! assert_eq!(format!("{:?}", vector), "[0, 1, 2, 3, 4]");
//! assert_eq!(format!("{:?}", vector2), "[5, 6]");
//!
//! vector.append(&mut vector2);
//!
//! assert_eq!(format!("{:?}", vector), "[0, 1, 2, 3, 4, 5, 6]");
//! assert_eq!(format!("{:?}", vector2), "[]");
//! ```
//!
//! # Iterator
//! ```
//! use arraydeque::ArrayDeque;
//!
//! let mut vector: ArrayDeque<[_; 8]> = ArrayDeque::new();
//!
//! vector.extend(0..5);
//!
//! let iters: Vec<_> = vector.into_iter().collect();
//! assert_eq!(iters, vec![0, 1, 2, 3, 4]);
//! ```
//!
//! # From Iterator
//! ```
//! use arraydeque::ArrayDeque;
//!
//! let vector: ArrayDeque<[_; 8]>;
//! let vector2: ArrayDeque<[_; 8]>;
//!
//! vector = vec![0, 1, 2, 3, 4].into_iter().collect();
//!
//! vector2 = (0..5).into_iter().collect();
//!
//! assert_eq!(vector, vector2);
//! ```

#![cfg_attr(not(any(feature="std", test)), no_std)]

#![deny(missing_docs)]

extern crate odds;
// #![cfg_attr(not(or(feature="std", test), no_std))]
#[cfg(not(any(feature="std", test)))]
extern crate core as std;

use std::mem;
use std::cmp;
use std::cmp::Ordering;
use std::mem::ManuallyDrop;
use std::hash::{Hash, Hasher};
use std::fmt;
use std::ptr;
use std::slice;
use std::iter;
use std::ops::Index;
use std::ops::IndexMut;
use std::marker::PhantomData;

pub use odds::IndexRange as RangeArgument;

mod array;
mod behavior;
pub mod error;

pub use array::Array;
pub use error::CapacityError;
use array::Index as ArrayIndex;

/// Tagging trait for providing behaviors to `ArrayDeque`.
pub trait Behavior {}

/// Behavior for `ArrayDeque` that specifies wrapping write semantics.
///
/// ### Pushing to back:
///
/// Pushing elements to the **back** of a fixed-size deque that **has already reached its capacity**
/// causes it to **overwrite** existing elements from the **front**.
///
/// ### Pushing to front:
///
/// Pushing elements to the **front** of a fixed-size deque that **has already reached its capacity**
/// causes it to **overwrite** existing elements from the **back**.
pub use behavior::{Behavior, Saturating, Wrapping};

unsafe fn new_array<A: Array>() -> A {
    // Note: Returning an uninitialized value here only works
    // if we can be sure the data is never used. The nullable pointer
    // inside enum optimization conflicts with this this for example,
    // so we need to be extra careful. See `NoDrop` enum.
    mem::uninitialized()
}

/// A fixed capacity ring buffer.
///
/// It can be stored directly on the stack if needed.
///
/// The "default" usage of this type as a queue is to use `push_back` to add to
/// the queue, and `pop_front` to remove from the queue. `extend` and `append`
/// push onto the back in this manner, and iterating over `ArrayDeque` goes front
/// to back.
///
/// # Capacity
///
/// Note that the `capacity()` is always `backed_array.len() - 1`.
/// [Read more]
///
/// [Read more]: https://en.wikipedia.org/wiki/Circular_buffer
pub struct ArrayDeque<A: Array, B: Behavior = Saturating> {
    xs: ManuallyDrop<A>,
    head: A::Index,
    tail: A::Index,
    phantom: PhantomData<B>,
}

impl<A: Array> Clone for ArrayDeque<A, Saturating>
    where A::Item: Clone
{
    fn clone(&self) -> Self {
        self.iter().cloned().collect()
    }
}

impl<A: Array> Clone for ArrayDeque<A, Wrapping>
    where A::Item: Clone
{
    fn clone(&self) -> Self {
        self.iter().cloned().collect()
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
        self.tail() <= self.head()
    }

    #[inline]
    fn head(&self) -> usize {
        self.head.to_usize()
    }

    #[inline]
    fn tail(&self) -> usize {
        self.tail.to_usize()
    }

    #[inline]
    unsafe fn set_head(&mut self, head: usize) {
        debug_assert!(head <= self.capacity());
        self.head = ArrayIndex::from(head);
    }

    #[inline]
    unsafe fn set_tail(&mut self, tail: usize) {
        debug_assert!(tail <= self.capacity());
        self.tail = ArrayIndex::from(tail);
    }

    /// Copies a contiguous block of memory len long from src to dst
    #[inline]
    unsafe fn copy(&mut self, dst: usize, src: usize, len: usize) {
        debug_assert!(dst + len <= A::capacity(),
                      "cpy dst={} src={} len={} cap={}",
                      dst,
                      src,
                      len,
                      A::capacity());
        debug_assert!(src + len <= A::capacity(),
                      "cpy dst={} src={} len={} cap={}",
                      dst,
                      src,
                      len,
                      A::capacity());
        ptr::copy(self.ptr_mut().offset(src as isize),
                  self.ptr_mut().offset(dst as isize),
                  len);
    }

    /// Copies a potentially wrapping block of memory len long from src to dest.
    /// (abs(dst - src) + len) must be no larger than cap() (There must be at
    /// most one continuous overlapping region between src and dest).
    unsafe fn wrap_copy(&mut self, dst: usize, src: usize, len: usize) {
        #[allow(dead_code)]
        fn diff(a: usize, b: usize) -> usize {
            if a <= b { b - a } else { a - b }
        }
        debug_assert!(cmp::min(diff(dst, src), A::capacity() - diff(dst, src)) + len <=
                      A::capacity(),
                      "wrc dst={} src={} len={} cap={}",
                      dst,
                      src,
                      len,
                      A::capacity());

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
    unsafe fn buffer_as_slice(&self) -> &[A::Item] {
        slice::from_raw_parts(self.ptr(), A::capacity())
    }

    #[inline]
    unsafe fn buffer_as_mut_slice(&mut self) -> &mut [A::Item] {
        slice::from_raw_parts_mut(self.ptr_mut(), A::capacity())
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
    /// let vector: ArrayDeque<[usize; 3]> = ArrayDeque::new();
    /// ```
    #[inline]
    pub fn new() -> Self {
        unsafe {
            ArrayDeque {
                xs: ManuallyDrop::new(mem::uninitialized()),
                head: ArrayIndex::from(0),
                tail: ArrayIndex::from(0),
                phantom: PhantomData,
            }
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
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// deque.push_back(3);
    /// deque.push_back(4);
    /// deque.push_back(5);
    /// assert_eq!(deque.get(1), Some(&4));
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
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// deque.push_back(3);
    /// deque.push_back(4);
    /// deque.push_back(5);
    /// if let Some(elem) = deque.get_mut(1) {
    ///     *elem = 7;
    /// }
    ///
    /// assert_eq!(deque[1], 7);
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
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// deque.push_back(3);
    /// deque.push_back(4);
    /// deque.push_back(5);
    /// deque.swap(0, 2);
    /// assert_eq!(deque[0], 5);
    /// assert_eq!(deque[2], 3);
    /// ```
    #[inline]
    pub fn swap(&mut self, i: usize, j: usize) {
        assert!(i < self.len());
        assert!(j < self.len());
        let ri = Self::wrap_add(self.tail(), i);
        let rj = Self::wrap_add(self.tail(), j);
        unsafe {
            ptr::swap(self.ptr_mut().offset(ri as isize),
                      self.ptr_mut().offset(rj as isize))
        }
    }

    /// Return the capacity of the `ArrayDeque`.
    ///
    /// # Capacity
    ///
    /// Note that the `capacity()` is always `backed_array.len() - 1`.
    /// [Read more]
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    /// let mut deque: ArrayDeque<[usize; 4]> = ArrayDeque::new();
    /// assert_eq!(deque.capacity(), 3);
    /// ```
    ///
    /// [Read more]: https://en.wikipedia.org/wiki/Circular_buffer
    #[inline]
    pub fn capacity(&self) -> usize {
        A::capacity() - 1
    }

    /// Returns a front-to-back iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// deque.push_back(5);
    /// deque.push_back(3);
    /// deque.push_back(4);
    /// let b: &[_] = &[&5, &3, &4];
    /// let c: Vec<&i32> = deque.iter().collect();
    /// assert_eq!(&c[..], b);
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<A::Item> {
        Iter {
            head: self.head(),
            tail: self.tail(),
            ring: unsafe { self.buffer_as_slice() },
        }
    }

    /// Returns a front-to-back iterator that returns mutable references.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// deque.push_back(5);
    /// deque.push_back(3);
    /// deque.push_back(4);
    /// for num in deque.iter_mut() {
    ///     *num = *num - 2;
    /// }
    /// let b: &[_] = &[&mut 3, &mut 1, &mut 2];
    /// assert_eq!(&deque.iter_mut().collect::<Vec<&mut i32>>()[..], b);
    /// ```
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<A::Item> {
        IterMut {
            head: self.head(),
            tail: self.tail(),
            ring: unsafe { self.buffer_as_mut_slice() },
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
    /// let mut vector: ArrayDeque<[_; 6]> = ArrayDeque::new();
    ///
    /// vector.push_back(0);
    /// vector.push_back(1);
    /// vector.push_back(2);
    ///
    /// assert_eq!(vector.as_slices(), (&[0, 1, 2][..], &[][..]));
    ///
    /// vector.push_front(10);
    /// vector.push_front(9);
    ///
    /// assert_eq!(vector.as_slices(), (&[9, 10][..], &[0, 1, 2][..]));
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
    /// let mut vector: ArrayDeque<[_; 5]> = ArrayDeque::new();
    ///
    /// vector.push_back(0);
    /// vector.push_back(1);
    ///
    /// vector.push_front(10);
    /// vector.push_front(9);
    ///
    /// vector.as_mut_slices().0[0] = 42;
    /// vector.as_mut_slices().1[0] = 24;
    /// assert_eq!(vector.as_slices(), (&[42, 10][..], &[24, 1][..]));
    /// ```
    #[inline]
    pub fn as_mut_slices(&mut self) -> (&mut [A::Item], &mut [A::Item]) {
        unsafe {
            let contiguous = self.is_contiguous();
            let head = self.head();
            let tail = self.tail();
            let buf = self.buffer_as_mut_slice();

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

    /// Returns the number of elements in the `ArrayDeque`.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut v: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// assert_eq!(v.len(), 0);
    /// v.push_back(1);
    /// assert_eq!(v.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        count(self.tail(), self.head(), A::capacity())
    }

    /// Returns true if the buffer contains no elements
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut v: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// assert!(v.is_empty());
    /// v.push_front(1);
    /// assert!(!v.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head() == self.tail()
    }

    /// Returns true if the buffer is full.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 2]> = ArrayDeque::new();
    ///
    /// assert!(!buf.is_full());
    ///
    /// buf.push_back(1);
    ///
    /// assert!(buf.is_full());
    /// ```
    #[inline]
    pub fn is_full(&self) -> bool {
        A::capacity() - self.len() == 1
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
    /// the end point is greater than the length of the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 4]> = vec![1, 2, 3].into_iter().collect();
    /// let drain1: Vec<_> = deque.drain(2..).collect();
    /// assert_eq!(drain1, vec![3]);
    ///
    /// // A full range clears all contents
    /// let drain2: Vec<_> = deque.drain(..).collect();
    /// assert_eq!(drain2, vec![1, 2]);
    /// assert!(deque.is_empty());
    /// ```
    pub fn drain<R>(&mut self, range: R) -> Drain<A, B>
        where R: RangeArgument<usize>
    {
        let len = self.len();
        let start = range.start().unwrap_or(0);
        let end = range.end().unwrap_or(len);
        assert!(start <= end, "drain lower bound was too large");
        assert!(end <= len, "drain upper bound was too large");

        let drain_tail = Self::wrap_add(self.tail(), start);
        let drain_head = Self::wrap_add(self.tail(), end);
        let head = self.head();

        unsafe { self.set_head(drain_tail) }

        Drain {
            deque: self as *mut _,
            after_tail: drain_head,
            after_head: head,
            iter: Iter {
                tail: drain_tail,
                head: drain_head,
                ring: unsafe { self.buffer_as_mut_slice() },
            },
        }
    }

    /// Clears the buffer, removing all values.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut v: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// v.push_back(1);
    /// v.clear();
    /// assert!(v.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.drain(..);
    }

    /// Returns `true` if the `ArrayDeque` contains an element equal to the
    /// given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut vector: ArrayDeque<[_; 3]> = ArrayDeque::new();
    ///
    /// vector.push_back(0);
    /// vector.push_back(1);
    ///
    /// assert_eq!(vector.contains(&1), true);
    /// assert_eq!(vector.contains(&10), false);
    /// ```
    pub fn contains(&self, x: &A::Item) -> bool
        where A::Item: PartialEq<A::Item>
    {
        let (a, b) = self.as_slices();
        a.contains(x) || b.contains(x)
    }

    /// Provides a reference to the front element, or `None` if the deque is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(deque.front(), None);
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// assert_eq!(deque.front(), Some(&1));
    /// ```
    pub fn front(&self) -> Option<&A::Item> {
        if !self.is_empty() {
            Some(&self[0])
        } else {
            None
        }
    }

    /// Provides a mutable reference to the front element, or `None` if the
    /// deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(deque.front_mut(), None);
    ///
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// match deque.front_mut() {
    ///     Some(x) => *x = 9,
    ///     None => (),
    /// }
    /// assert_eq!(deque.front(), Some(&9));
    /// ```
    pub fn front_mut(&mut self) -> Option<&mut A::Item> {
        if !self.is_empty() {
            Some(&mut self[0])
        } else {
            None
        }
    }

    /// Provides a reference to the back element, or `None` if the deque is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(deque.back(), None);
    ///
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// assert_eq!(deque.back(), Some(&2));
    /// ```
    pub fn back(&self) -> Option<&A::Item> {
        if !self.is_empty() {
            Some(&self[self.len() - 1])
        } else {
            None
        }
    }

    /// Provides a mutable reference to the back element, or `None` if the
    /// deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(deque.back(), None);
    ///
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// match deque.back_mut() {
    ///     Some(x) => *x = 9,
    ///     None => (),
    /// }
    /// assert_eq!(deque.back(), Some(&9));
    /// ```
    pub fn back_mut(&mut self) -> Option<&mut A::Item> {
        let len = self.len();
        if !self.is_empty() {
            Some(&mut self[len - 1])
        } else {
            None
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
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// assert_eq!(deque.swap_remove_back(0), None);
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// deque.push_back(3);
    ///
    /// assert_eq!(deque.swap_remove_back(0), Some(1));
    /// assert_eq!(deque.len(), 2);
    /// assert_eq!(deque[0], 3);
    /// assert_eq!(deque[1], 2);
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
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// assert_eq!(deque.swap_remove_front(0), None);
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// deque.push_back(3);
    ///
    /// assert_eq!(deque.swap_remove_front(2), Some(3));
    /// assert_eq!(deque.len(), 2);
    /// assert_eq!(deque[0], 2);
    /// assert_eq!(deque[1], 1);
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

    #[inline]
    fn push_front_expecting_space_available(&mut self, element: A::Item) {
        debug_assert!(!self.is_full());
        unsafe {
            let new_tail = Self::wrap_sub(self.tail(), 1);
            self.set_tail(new_tail);
            self.buffer_write(new_tail, element);
        }
    }

    #[inline]
    fn push_back_expecting_space_available(&mut self, element: A::Item) {
        debug_assert!(!self.is_full());
        unsafe {
            let head = self.head();
            self.set_head(Self::wrap_add(head, 1));
            self.buffer_write(head, element);
        }
    }

    #[inline]
    fn insert_expecting_space_available(&mut self, index: usize, element: A::Item) {
        debug_assert!(!self.is_full());

        assert!(index <= self.len(), "index out of bounds");

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

        match (contiguous, distance_to_tail <= distance_to_head, idx >= self.tail()) {
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

                let new_tail = Self::wrap_sub(self.tail(), 1);
                unsafe { self.set_tail(new_tail) }
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

                    self.set_tail(new_tail);
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
                    let new_head = Self::wrap_add(self.head(), 1);
                    self.set_head(new_head);
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
                    self.set_tail(tail - 1);
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

                    self.set_head(head + 1);
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

                    self.set_tail(tail - 1);
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

                    self.set_tail(tail - 1);
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
                    self.set_head(head + 1);
                }
            }
        }

        // tail might've been changed so we need to recalculate
        let new_idx = Self::wrap_add(self.tail(), index);
        unsafe {
            self.buffer_write(new_idx, element);
        }
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
    /// let mut deque: ArrayDeque<[_; 4]> = vec![1,2,3].into_iter().collect();
    /// let buf2 = deque.split_off(1);
    /// // deque = [1], buf2 = [2, 3]
    /// assert_eq!(deque.len(), 1);
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

                ptr::copy_nonoverlapping(first_half.as_ptr().offset(at as isize),
                                         other.ptr_mut(),
                                         amount_in_first);

                // just take all of the second half.
                ptr::copy_nonoverlapping(second_half.as_ptr(),
                                         other.ptr_mut().offset(amount_in_first as isize),
                                         second_len);
            } else {
                // `at` lies in the second half, need to factor in the elements we skipped
                // in the first half.
                let offset = at - first_len;
                let amount_in_second = second_len - offset;
                ptr::copy_nonoverlapping(second_half.as_ptr().offset(offset as isize),
                                         other.ptr_mut(),
                                         amount_in_second);
            }
        }

        // Cleanup where the ends of the buffers are
        unsafe {
            let head = self.head();
            self.set_head(Self::wrap_sub(head, other_len));
            other.set_head(other_len);
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
    /// let mut deque: ArrayDeque<[_; 5]> = ArrayDeque::new();
    /// deque.extend(1..5);
    /// deque.retain(|&x| x%2 == 0);
    ///
    /// let v: Vec<_> = deque.into_iter().collect();
    /// assert_eq!(&v[..], &[2, 4]);
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
        where F: FnMut(&A::Item) -> bool
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

    /// Removes and returns an element from the front of the deque.
    ///
    /// Returns the element, or `None` if the deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// deque.push_back(1);
    /// deque.push_back(2);
    ///
    /// assert_eq!(deque.pop_front(), Some(1));
    /// assert_eq!(deque.pop_front(), Some(2));
    /// assert_eq!(deque.pop_front(), None);
    /// ```
    pub fn pop_front(&mut self) -> Option<A::Item> {
        if self.is_empty() {
            return None;
        }
        unsafe {
            let tail = self.tail();
            self.set_tail(Self::wrap_add(tail, 1));
            Some(self.buffer_read(tail))
            }
        }

    /// Removes an element from the back of the deque.
    ///
    /// Returns the element, or `None` if the deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(deque.pop_back(), None);
    /// deque.push_back(1);
    /// deque.push_back(3);
    /// assert_eq!(deque.pop_back(), Some(3));
    /// ```
    pub fn pop_back(&mut self) -> Option<A::Item> {
        if self.is_empty() {
            return None;
            }
        unsafe {
            let new_head = Self::wrap_sub(self.head(), 1);
            self.set_head(new_head);
            Some(self.buffer_read(new_head))
        }
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
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// deque.push_back(3);
    ///
    /// assert_eq!(deque.remove(1), Some(2));
    /// assert_eq!(deque.get(1), Some(&3));
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

        match (contiguous, distance_to_tail <= distance_to_head, idx >= self.tail()) {
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
                    self.set_tail(tail + 1);
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
                    self.set_head(head - 1);
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
                    let new_tail = Self::wrap_add(self.tail(), 1);
                    self.set_tail(new_tail);
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
                    self.set_head(head - 1);
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

                    let new_head = Self::wrap_sub(self.head(), 1);
                    self.set_head(new_head);
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

                    let new_tail = Self::wrap_add(tail, 1);
                    self.set_tail(new_tail);
                }
            }
        }

        return elem;
    }
}

impl<A: Array> ArrayDeque<A, Saturating> {
    /// Converts `self` into a `ArrayDeque<A, Saturating>`
    pub fn wrapping(mut self) -> ArrayDeque<A, Wrapping> {
        use std::iter::FromIterator;
        ArrayDeque::<A, Wrapping>::from_iter(self.drain(..))
    }

    /// Adds an element to the front of the deque.
    ///
    /// Return `None` if the push succeeds, or `Some(element)`
    /// if the vector is full.
    ///
    /// # Examples
    ///
    /// ```text
    /// 1 -(+)-> [_, _, _] => [1, _, _] -> None
    /// 2 -(+)-> [1, _, _] => [2, 1, _] -> None
    /// 3 -(+)-> [2, 1, _] => [3, 2, 1] -> None
    /// 4 -(+)-> [3, 2, 1] => [3, 2, 1] -> Some(4)
    /// ```
    ///
    /// ```
    /// use arraydeque::{ArrayDeque, Saturating};
    ///
    /// let mut deque: ArrayDeque<[_; 3], Saturating> = ArrayDeque::new();
    /// deque.push_front(1);
    /// deque.push_front(2);
    /// let overflow = deque.push_front(3);
    ///
    /// assert_eq!(deque.front(), Some(&2));
    /// assert_eq!(overflow, Some(3));
    /// ```
    pub fn push_front(&mut self, element: A::Item) -> Option<A::Item> {
        if !self.is_full() {
            self.push_front_expecting_space_available(element);
            None
        } else {
            Some(element)
        }
    }

    /// Adds an element to the back of the deque.
    ///
    /// Return `None` if the push succeeds, or `Some(element)`
    /// if the vector is full.
    ///
    /// # Examples
    ///
    /// ```text
    /// [_, _, _] <-(+)- 1 => [_, _, 1] -> None
    /// [_, _, 1] <-(+)- 2 => [_, 1, 2] -> None
    /// [_, 1, 2] <-(+)- 3 => [1, 2, 3] -> None
    /// [1, 2, 3] <-(+)- 4 => [1, 2, 3] -> Some(4)
    /// ```
    ///
    /// ```
    /// use arraydeque::{ArrayDeque, Saturating};
    ///
    /// let mut deque: ArrayDeque<[_; 3], Saturating> = ArrayDeque::new();
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// let overflow = deque.push_back(3);
    ///
    /// assert_eq!(deque.back(), Some(&2));
    /// assert_eq!(overflow, Some(3));
    /// ```
    pub fn push_back(&mut self, element: A::Item) -> Option<A::Item> {
        if !self.is_full() {
            self.push_back_expecting_space_available(element);
            None
        } else {
            Some(element)
    }
    }

    /// Inserts an element at `index` within the `ArrayDeque`. Whichever
    /// end is closer to the insertion point will be moved to make room,
    /// and all the affected elements will be moved to new positions.
    ///
    /// Return `None` if the push succeeds, or `Some(element)`
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
    ///
    /// ```text
    /// [0, _, _] <-(+)- 1 @ 0 => [1, 0, _] -> None
    /// [1, 0, _] <-(+)- 3 @ 1 => [1, 3, 0] -> None
    /// [1, 3, 0] <-(+)- 2 @ 1 => [1, 3, 0] -> Some(2)
    /// ```
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// deque.push_back(10);
    /// deque.push_back(12);
    /// deque.insert(1, 11);
    /// let overflow = deque.insert(0, 9);
    ///
    /// assert_eq!(Some(&11), deque.get(1));
    /// assert_eq!(overflow, Some(9));
    /// ```
    pub fn insert(&mut self, index: usize, element: A::Item) -> Option<A::Item> {
        if !self.is_full() {
            self.insert_expecting_space_available(index, element);
            None
        } else {
            Some(element)
        }
    }

    /// Iterates over the iterator's items, adding them to the front of the deque, one by one.
    ///
    /// Does not extract more items than there is space for.
    ///
    /// # Examples
    ///
    /// ```text
    /// [2, 1] -(+)-> [_, _, _] => [1, 2, _]
    /// [2, 1] -(+)-> [3, _, _] => [1, 2, 3]
    /// [2, 1] -(+)-> [3, 4, _] => [2, 3, 4]
    /// [2, 1] -(+)-> [3, 4, 5] => [3, 4, 5]
    /// ```
    ///
    /// ```
    /// use arraydeque::{ArrayDeque, Saturating};
    ///
    /// let mut deque: ArrayDeque<[_; 8], Saturating> = vec![7, 8, 9].into_iter().collect();
    /// let mut vec1 = vec![6, 5, 4];
    /// let mut vec2 = vec![3, 2, 1];
    ///
    /// deque.extend_front(vec1);
    /// assert_eq!(deque.len(), 6);
    ///
    /// // max capacity reached
    /// deque.extend_front(vec2);
    /// assert_eq!(deque.len(), 7);
    /// let collected: Vec<_> = deque.into_iter().collect();
    /// let expected = vec![3, 4, 5, 6, 7, 8, 9];
    /// assert_eq!(collected, expected);
    /// ```
    pub fn extend_front<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
        let take = self.capacity() - self.len();
        for element in iter.into_iter().take(take) {
            self.push_front(element);
        }
    }

    /// Iterates over the iterator's items, adding them to the back of the deque, one by one.
    ///
    /// Does not extract more items than there is space for.
    ///
    /// # Examples
    ///
    /// ```text
    /// [_, _, _] <-(+)- [2, 1] => [_, 2, 1]
    /// [_, _, 3] <-(+)- [2, 1] => [3, 2, 1]
    /// [_, 4, 3] <-(+)- [2, 1] => [4, 3, 2]
    /// [5, 4, 3] <-(+)- [2, 1] => [5, 4, 3]
    /// ```
    ///
    /// ```
    /// use arraydeque::{ArrayDeque, Saturating};
    ///
    /// let mut deque: ArrayDeque<[_; 8], Saturating> = vec![7, 8, 9].into_iter().collect();
    /// let mut vec1 = vec![6, 5, 4];
    /// let mut vec2 = vec![3, 2, 1];
    ///
    /// deque.extend_front(vec1);
    /// assert_eq!(deque.len(), 6);
    ///
    /// // max capacity reached
    /// deque.extend_front(vec2);
    /// assert_eq!(deque.len(), 7);
    /// let collected: Vec<_> = deque.into_iter().collect();
    /// let expected = vec![3, 4, 5, 6, 7, 8, 9];
    /// assert_eq!(collected, expected);
    /// ```
    pub fn extend_back<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
        let take = self.capacity() - self.len();
        for element in iter.into_iter().take(take) {
            self.push_back(element);
        }
    }

    /// Prepends `self` with `other`'s items, preserving their order, leaving `other` empty.
    ///
    /// Does not extract more items than there is space for. No error
    /// occurs if there are more iterator elements.
    ///
    /// # Examples
    ///
    /// ```text
    /// [1, 2] -(+)-> [_, _, _] => [1, 2, _]
    /// [1, 2] -(+)-> [3, _, _] => [1, 2, 3]
    /// [1, 2] -(+)-> [3, 4, _] => [2, 3, 4]
    /// [1, 2] -(+)-> [3, 4, 5] => [3, 4, 5]
    /// ```
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 8]> = vec![1, 2, 3].into_iter().collect();
    /// let mut buf2: ArrayDeque<[_; 8]> = vec![4, 5, 6].into_iter().collect();
    /// let mut buf3: ArrayDeque<[_; 8]> = vec![7, 8, 9].into_iter().collect();
    ///
    /// deque.prepend(&mut buf2);
    /// assert_eq!(deque.len(), 6);
    /// assert_eq!(buf2.len(), 0);
    ///
    /// // max capacity reached
    /// deque.prepend(&mut buf3);
    /// assert_eq!(deque.len(), 7);
    /// assert_eq!(buf3.len(), 0);
    /// ```
    pub fn prepend<T: Array<Item=A::Item>, B: Behavior>(&mut self, other: &mut ArrayDeque<T, B>) {
        let take = self.capacity() - self.len();
        for element in other.drain(..).into_iter().rev().take(take) {
            self.push_front(element);
        }
    }

    /// Appends `self` with `other`'s items, preserving their order, leaving `other` empty.
    ///
    /// Does not extract more items than there is space for. No error
    /// occurs if there are more iterator elements.
    ///
    /// # Examples
    ///
    /// ```text
    /// [_, _, _] <-(+)- [2, 1] => [_, 2, 1]
    /// [_, _, 3] <-(+)- [2, 1] => [3, 2, 1]
    /// [_, 4, 3] <-(+)- [2, 1] => [4, 3, 2]
    /// [5, 4, 3] <-(+)- [2, 1] => [5, 4, 3]
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 8]> = vec![1, 2, 3].into_iter().collect();
    /// let mut buf2: ArrayDeque<[_; 8]> = vec![4, 5, 6].into_iter().collect();
    /// let mut buf3: ArrayDeque<[_; 8]> = vec![7, 8, 9].into_iter().collect();
    ///
    /// deque.append(&mut buf2);
    /// assert_eq!(deque.len(), 6);
    /// assert_eq!(buf2.len(), 0);
    ///
    /// // max capacity reached
    /// deque.append(&mut buf3);
    /// assert_eq!(deque.len(), 7);
    /// assert_eq!(buf3.len(), 0);
    /// ```
    pub fn append<T: Array<Item=A::Item>, B: Behavior>(&mut self, other: &mut ArrayDeque<T, B>) {
        let take = self.capacity() - self.len();
        for element in other.drain(..).into_iter().take(take) {
            self.push_back(element);
        }
    }
}

impl<A: Array> ArrayDeque<A, Wrapping> {
    /// Converts `self` into a `ArrayDeque<A, Wrapping>`
    pub fn saturating(mut self) -> ArrayDeque<A, Saturating> {
        use std::iter::FromIterator;
        ArrayDeque::<A, Saturating>::from_iter(self.drain(..))
    }

    /// Adds an element to the front of the deque.
    ///
    /// Return `None` if the still had capacity, or `Some(existing)`
    /// if the vector is full, where `existing` is the element being overwritten.
    ///
    /// # Examples
    ///
    /// ```text
    /// 1 -(+)-> [_, _, _] => [1, _, _] -> None
    /// 2 -(+)-> [1, _, _] => [2, 1, _] -> None
    /// 3 -(+)-> [2, 1, _] => [3, 2, 1] -> None
    /// 4 -(+)-> [3, 2, 1] => [4, 3, 2] -> Some(1)
    /// ```
    /// use arraydeque::{ArrayDeque, Wrapping};
    ///
    /// let mut deque: ArrayDeque<[_; 3], Wrapping> = ArrayDeque::new();
    /// deque.push_front(1);
    /// deque.push_front(2);
    /// let overflow = deque.push_front(3);
    ///
    /// assert_eq!(deque.front(), Some(&3));
    /// assert_eq!(overflow, Some(1));
    /// ```
    pub fn push_front(&mut self, element: A::Item) -> Option<A::Item> {
        let existing = if self.is_full() {
            self.pop_back()
            } else {
            None
        };
        self.push_front_expecting_space_available(element);
        existing
        }

    /// Adds an element to the back of the deque.
    ///
    /// Return `None` if the still had capacity, or `Some(existing)`
    /// if the vector is full, where `existing` is the element being overwritten.
    ///
    /// # Examples
    ///
    /// ```text
    /// [_, _, _] <-(+)- 1 => [_, _, 1] -> None
    /// [_, _, 1] <-(+)- 2 => [_, 1, 2] -> None
    /// [_, 1, 2] <-(+)- 3 => [1, 2, 3] -> None
    /// [1, 2, 3] <-(+)- 4 => [2, 3, 4] -> Some(1)
    /// ```
    ///
    /// ```
    /// use arraydeque::{ArrayDeque, Wrapping};
    ///
    /// let mut deque: ArrayDeque<[_; 3], Wrapping> = ArrayDeque::new();
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// let overflow = deque.push_back(3);
    ///
    /// assert_eq!(deque.back(), Some(&3));
    /// assert_eq!(overflow, Some(1));
    /// ```
    pub fn push_back(&mut self, element: A::Item) -> Option<A::Item> {
        let existing = if self.is_full() {
            self.pop_front()
        } else {
            None
        };
        self.push_back_expecting_space_available(element);
        existing
        }

    /// Inserts an element at `index` within the `ArrayDeque`. Whichever
    /// end is closer to the insertion point will be moved to make room,
    /// and all the affected elements will be moved to new positions.
    ///
    /// Return `None` if the still had capacity, or `Some(existing)`
    /// if the vector is full, where `existing` is the frontmost element being overwritten.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than `ArrayDeque`'s length
    ///
    /// # Examples
    ///
    /// ```text
    /// [0, _, _] <-(+)- 1 @ 0 => [1, 0, _] -> None
    /// [1, 0, _] <-(+)- 3 @ 1 => [1, 3, 0] -> None
    /// [1, 3, 0] <-(+)- 2 @ 1 => [1, 2, 3] -> Some(0)
    /// ```
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// deque.push_back(10);
    /// deque.push_back(12);
    /// deque.insert(1, 11);
    /// let overflow = deque.insert(0, 9);
    ///
    /// assert_eq!(Some(&11), deque.get(1));
    /// assert_eq!(overflow, Some(9));
    /// ```
    pub fn insert(&mut self, index: usize, element: A::Item) -> Option<A::Item> {
        let existing = if self.is_full() {
            self.pop_front()
        } else {
            None
        };
        self.insert_expecting_space_available(index, element);
        existing
    }

    /// Iterates over the iterator's items, adding them to the front of the deque, one by one.
    ///
    /// Extracts all items from `other` overwriting `self` if necessary.
    ///
    /// # Examples
    ///
    /// ```text
    /// [2, 1] -(+)-> [_, _, _] => [1, 2, _]
    /// [2, 1] -(+)-> [3, _, _] => [1, 2, 3]
    /// [2, 1] -(+)-> [3, 4, _] => [1, 2, 3]
    /// [2, 1] -(+)-> [3, 4, 5] => [1, 2, 3]
    /// ```
    ///
    /// ```
    /// use arraydeque::{ArrayDeque, Wrapping};
    ///
    /// let mut deque: ArrayDeque<[_; 8], Wrapping> = vec![7, 8, 9].into_iter().collect();
    /// let mut vec1 = vec![6, 5, 4];
    /// let mut vec2 = vec![3, 2, 1];
    ///
    /// deque.extend_front(vec1);
    /// assert_eq!(deque.len(), 6);
    ///
    /// // max capacity reached
    /// deque.extend_front(vec2);
    /// assert_eq!(deque.len(), 7);
    /// let collected: Vec<_> = deque.into_iter().collect();
    /// let expected = vec![1, 2, 3, 4, 5, 6, 7];
    /// assert_eq!(collected, expected);
    /// ```
    pub fn extend_front<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
        for element in iter.into_iter() {
            self.push_front(element);
        }
    }

    /// Iterates over the iterator's items, adding them to the back of the deque, one by one.
    /// 
    /// Extracts all items from `other` overwriting `self` if necessary.
    /// 
    /// # Examples
    /// 
    /// ```text
    /// [_, _, _] <-(+)- [2, 1] => [_, 2, 1]
    /// [_, _, 3] <-(+)- [2, 1] => [3, 2, 1]
    /// [_, 4, 3] <-(+)- [2, 1] => [3, 2, 1]
    /// [5, 4, 3] <-(+)- [2, 1] => [3, 2, 1]
    /// ```
    ///
    /// ```
    /// // use arraydeque::{ArrayDeque, Wrapping};
    /// //
    /// // let mut deque: ArrayDeque<[_; 8], Wrapping> = vec![9, 8, 7].into_iter().collect();
    /// // let mut vec1 = vec![6, 5, 4];
    /// // let mut vec2 = vec![3, 2, 1];
    /// //
    /// // deque.extend_back(vec1);
    /// // assert_eq!(deque.len(), 6);
    /// //
    /// // // max capacity reached
    /// // deque.extend_back(vec2);
    /// // assert_eq!(deque.len(), 7);
    /// // let collected: Vec<_> = deque.into_iter().collect();
    /// // let expected = vec![7, 6, 5, 4, 3, 2, 1];
    /// // assert_eq!(collected, expected);
    /// ```
    pub fn extend_back<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
        for element in iter.into_iter() {
            self.push_back(element);
        }
    }

    /// Prepends `self` with `other`'s items, preserving their order, leaving `other` empty.
    ///
    /// Extracts all items from `other` overwriting `self` if necessary.
    ///
    /// # Examples
    ///
    /// ```text
    /// [1, 2] -(+)-> [_, _, _] => [1, 2, _]
    /// [1, 2] -(+)-> [3, _, _] => [1, 2, 3]
    /// [1, 2] -(+)-> [3, 4, _] => [1, 2, 3]
    /// [1, 2] -(+)-> [3, 4, 5] => [1, 2, 3]
    /// ```
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 8]> = vec![1, 2, 3].into_iter().collect();
    /// let mut buf2: ArrayDeque<[_; 8]> = vec![4, 5, 6].into_iter().collect();
    /// let mut buf3: ArrayDeque<[_; 8]> = vec![7, 8, 9].into_iter().collect();
    ///
    /// deque.prepend(&mut buf2);
    /// assert_eq!(deque.len(), 6);
    /// assert_eq!(buf2.len(), 0);
    ///
    /// // max capacity reached
    /// deque.prepend(&mut buf3);
    /// assert_eq!(deque.len(), 7);
    /// assert_eq!(buf3.len(), 0);
    /// ```
    pub fn prepend<T: Array<Item=A::Item>, B: Behavior>(&mut self, other: &mut ArrayDeque<T, B>) {
        for element in other.drain(..).into_iter().rev() {
            self.push_front(element);
            }
        }

    /// Appends `self` with `other`'s items, preserving their order, leaving `other` empty.
    ///
    /// Extracts all items from `other` overwriting `self` if necessary.
    ///
    /// # Examples
    ///
    /// ```text
    /// [_, _, _] <-(+)- [2, 1] => [_, 2, 1]
    /// [_, _, 3] <-(+)- [2, 1] => [3, 2, 1]
    /// [_, 4, 3] <-(+)- [2, 1] => [3, 2, 1]
    /// [5, 4, 3] <-(+)- [2, 1] => [3, 2, 1]
    /// ```
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<[_; 8]> = vec![1, 2, 3].into_iter().collect();
    /// let mut buf2: ArrayDeque<[_; 8]> = vec![4, 5, 6].into_iter().collect();
    /// let mut buf3: ArrayDeque<[_; 8]> = vec![7, 8, 9].into_iter().collect();
    ///
    /// deque.append(&mut buf2);
    /// assert_eq!(deque.len(), 6);
    /// assert_eq!(buf2.len(), 0);
    ///
    /// // max capacity reached
    /// deque.append(&mut buf3);
    /// assert_eq!(deque.len(), 7);
    /// assert_eq!(buf3.len(), 0);
    /// ```
    pub fn append<T: Array<Item=A::Item>, B: Behavior>(&mut self, other: &mut ArrayDeque<T, B>) {
        for element in other.drain(..).into_iter() {
            self.push_back(element);
            }
        }
    }

impl<A: Array, B: Behavior> PartialEq for ArrayDeque<A, B>
    where A::Item: PartialEq
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
            // self:  [a b c|deque e f]
            // other: [0 1 2 3|4 5]
            // front = 3, mid = 1,
            // [a b c] == [0 1 2] && [deque] == [3] && [e f] == [4 5]
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

#[cfg(test)]
impl<'a, A: Array, B: Behavior> PartialEq<&'a [A::Item]> for ArrayDeque<A, B> where A::Item: PartialEq {
    fn eq(&self, other: &&'a [A::Item]) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter().zip(other.iter()).all(|(l, r)| l == r)
    }
        }

#[cfg(test)]
impl<A: Array, B: Behavior> PartialEq<Vec<A::Item>> for ArrayDeque<A, B> where A::Item: PartialEq {
    fn eq(&self, other: &Vec<A::Item>) -> bool {
        *self == &other[..]
    }
}

impl<A: Array, B: Behavior> Eq for ArrayDeque<A, B> where A::Item: Eq {}

impl<A: Array, B: Behavior> PartialOrd for ArrayDeque<A, B>
    where A::Item: PartialOrd
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<A: Array, B: Behavior> Ord for ArrayDeque<A, B>
    where A::Item: Ord
{
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<A: Array, B: Behavior> Hash for ArrayDeque<A, B>
    where A::Item: Hash
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
                panic!("index out of bounds: the len is {} but the index is {}",
                       len,
                       index)
            })
            .unwrap()
    }
}

impl<A: Array, B: Behavior> IndexMut<usize> for ArrayDeque<A, B> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut A::Item {
        let len = self.len();
        self.get_mut(index)
            .or_else(|| {
                panic!("index out of bounds: the len is {} but the index is {}",
                       len,
                       index)
            })
            .unwrap()
    }
}

impl<A: Array> iter::FromIterator<A::Item> for ArrayDeque<A, Saturating> {
    fn from_iter<T: IntoIterator<Item = A::Item>>(iter: T) -> Self {
        let mut array = ArrayDeque::new();
        array.extend(iter);
        array
    }
}

impl<A: Array> iter::FromIterator<A::Item> for ArrayDeque<A, Wrapping> {
    fn from_iter<T: IntoIterator<Item = A::Item>>(iter: T) -> Self {
        let mut array = ArrayDeque::new();
        array.extend(iter);
        array
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

/// Extend the `ArrayDeque` with an iterator.
///
/// Does not extract more items than there is space for. No error
/// occurs if there are more iterator elements.
impl<A: Array> Extend<A::Item> for ArrayDeque<A, Saturating> {
    fn extend<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
        let take = self.capacity() - self.len();
        for elt in iter.into_iter().take(take) {            
            self.push_back(elt);
        }
    }
}

/// Extend the `ArrayDeque` with an iterator.
///
/// Does not extract more items than there is space for. No error
/// occurs if there are more iterator elements.
impl<A: Array> Extend<A::Item> for ArrayDeque<A, Wrapping> {
    fn extend<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
        for elt in iter.into_iter() {
            self.push_back(elt);
        }
    }
}

impl<A: Array, B: Behavior> fmt::Debug for ArrayDeque<A, B>
    where A::Item: fmt::Debug
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

#[inline]
fn count(tail: usize, head: usize, capacity: usize) -> usize {
    debug_assert!(head < capacity);
    debug_assert!(tail < capacity);
    if head >= tail {
        head - tail
    } else {
        capacity + head - tail
    }
}

/// `ArrayDeque` iterator
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct Iter<'a, T: 'a> {
    ring: &'a [T],
    head: usize,
    tail: usize,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        if self.tail == self.head {
            return None;
        }
        let tail = self.tail;
        self.tail = wrap_add(self.tail, 1, self.ring.len());
        unsafe { Some(self.ring.get_unchecked(tail)) }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = count(self.tail, self.head, self.ring.len());
        (len, Some(len))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a T> {
        if self.tail == self.head {
            return None;
        }
        self.head = wrap_sub(self.head, 1, self.ring.len());
        unsafe { Some(self.ring.get_unchecked(self.head)) }
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {}

/// `ArrayDeque` mutable iterator
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct IterMut<'a, T: 'a> {
    ring: &'a mut [T],
    head: usize,
    tail: usize,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<&'a mut T> {
        if self.tail == self.head {
            return None;
        }
        let tail = self.tail;
        self.tail = wrap_add(self.tail, 1, self.ring.len());

        unsafe {
            let elem = self.ring.get_unchecked_mut(tail);
            Some(&mut *(elem as *mut _))
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = count(self.tail, self.head, self.ring.len());
        (len, Some(len))
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut T> {
        if self.tail == self.head {
            return None;
        }
        self.head = wrap_sub(self.head, 1, self.ring.len());

        unsafe {
            let elem = self.ring.get_unchecked_mut(self.head);
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
    where A: Array,
          A::Item: 'a,
          B: Behavior,
{
    after_tail: usize,
    after_head: usize,
    iter: Iter<'a, A::Item>,
    deque: *mut ArrayDeque<A, B>,
}

impl<'a, A, B> Drop for Drain<'a, A, B>
    where A: Array,
          A::Item: 'a,
          B: Behavior,
{
    fn drop(&mut self) {
        for _ in self.by_ref() {}

        let source_deque = unsafe { &mut *self.deque };

        // T = source_deque_tail; H = source_deque_head; t = drain_tail; h = drain_head
        //
        //        T   t   h   H
        // [. . . o o x x o o . . .]
        //
        let orig_tail = source_deque.tail();
        let drain_tail = source_deque.head();
        let drain_head = self.after_tail;
        let orig_head = self.after_head;

        let tail_len = count(orig_tail, drain_tail, A::capacity());
        let head_len = count(drain_head, orig_head, A::capacity());

        // Restore the original head value
        unsafe { source_deque.set_head(orig_head) }
        match (tail_len, head_len) {
            (0, 0) => {
                unsafe { source_deque.set_head(0) }
                unsafe { source_deque.set_tail(0) }
            }
            (0, _) => unsafe { source_deque.set_tail(drain_head) },
            (_, 0) => unsafe { source_deque.set_head(drain_tail) },
            _ => unsafe {
                if tail_len <= head_len {
                    let new_tail = ArrayDeque::<A, B>::wrap_sub(drain_head, tail_len);
                    source_deque.set_tail(new_tail);
                    source_deque.wrap_copy(new_tail, orig_tail, tail_len);
                } else {
                    let new_head = ArrayDeque::<A, B>::wrap_add(drain_tail, head_len);
                    source_deque.set_head(new_head);
                    source_deque.wrap_copy(drain_tail, drain_head, head_len);
                }
            },
        }
    }
}

impl<'a, A, B> Iterator for Drain<'a, A, B>
    where A: Array,
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
    where A: Array,
          A::Item: 'a,
          B: Behavior
{
    #[inline]
    fn next_back(&mut self) -> Option<A::Item> {
        self.iter.next_back().map(|elt| unsafe { ptr::read(elt) })
    }
}

impl<'a, A, B> ExactSizeIterator for Drain<'a, A, B>
    where A: Array,
          A::Item: 'a,
          B: Behavior
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;
    use std::iter::FromIterator;

    #[test]
    fn wrapping_from_iter() {
        type Deque = ArrayDeque<[usize; 5], Wrapping>;

        // within capacity:
        assert_eq!(Deque::from_iter(vec![1, 2, 3]), vec![1, 2, 3]);

        // beyond capacity:
        assert_eq!(Deque::from_iter(vec![1, 2, 3, 4, 5]), vec![2, 3, 4, 5]);
    }

    #[test]
    fn saturating_from_iter() {
        type Deque = ArrayDeque<[usize; 5], Saturating>;

        // within capacity:
        assert_eq!(Deque::from_iter(vec![1, 2, 3]), vec![1, 2, 3]);

        // beyond capacity:
        assert_eq!(Deque::from_iter(vec![1, 2, 3, 4, 5]), vec![1, 2, 3, 4]);
    }

    #[test]
    fn saturating_wrapping() {
        type SaturatingDeque = ArrayDeque<[usize; 6], Saturating>;

        let saturating = SaturatingDeque::from_iter(vec![1, 2, 3]);
        assert_eq!(saturating.wrapping(), vec![1, 2, 3]);
    }

    #[test]
    fn wrapping_saturating() {
        type WrappingDeque = ArrayDeque<[usize; 6], Wrapping>;

        let wrapping = WrappingDeque::from_iter(vec![1, 2, 3]);
        assert_eq!(wrapping.saturating(), vec![1, 2, 3]);
    }

    #[test]
    fn any_simple() {
        macro_rules! test {
            ($behavior:ident) => ({
                let mut tester: ArrayDeque<[_; 8], $behavior> = ArrayDeque::new();
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
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_simple_reversely() {
        macro_rules! test {
            ($behavior:ident) => ({
                let mut tester: ArrayDeque<[_; 8], $behavior> = ArrayDeque::new();
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
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn saturating_overflow() {
        let mut tester: ArrayDeque<[_; 3], Saturating> = ArrayDeque::new();
        assert_eq!(tester.push_back(1), None);
        assert_eq!(tester.push_back(2), None);
        assert_eq!(tester.push_back(3), Some(3));
    }

    #[test]
    fn wrapping_overflow() {
        let mut tester: ArrayDeque<[_; 3], Wrapping> = ArrayDeque::new();
        assert_eq!(tester.push_back(1), None);
        assert_eq!(tester.push_back(2), None);
        assert_eq!(tester.push_back(3), Some(1));
    }

    #[test]
    fn any_pop_empty() {
        macro_rules! test {
            ($behavior:ident) => ({
                let mut tester: ArrayDeque<[_; 3], $behavior> = ArrayDeque::new();
                assert_eq!(tester.push_back(1), None);
        assert_eq!(tester.pop_front(), Some(1));
        assert_eq!(tester.is_empty(), true);
        assert_eq!(tester.len(), 0);
        assert_eq!(tester.pop_front(), None);
            })
    }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_index() {
        macro_rules! test {
            ($behavior:ident) => ({
                let mut tester: ArrayDeque<[_; 4], $behavior> = ArrayDeque::new();
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
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    #[should_panic]
    fn any_index_overflow() {
        macro_rules! test {
            ($behavior:ident) => ({
                let mut tester: ArrayDeque<[_; 4], $behavior> = ArrayDeque::new();
        tester.push_back(1);
        tester.push_back(2);
        tester[2];
            })
    }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_iter() {
        macro_rules! test {
            ($behavior:ident) => ({
                let mut tester: ArrayDeque<[_; 3], $behavior> = ArrayDeque::new();
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
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_iter_mut() {
        macro_rules! test {
            ($behavior:ident) => ({
                let mut tester: ArrayDeque<[_; 3], $behavior> = ArrayDeque::new();
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
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_into_iter() {
        #[derive(Eq, PartialEq, Debug)]
        struct NoCopy<T>(T);

        macro_rules! test {
            ($behavior:ident) => ({
        {
                    let mut tester: ArrayDeque<[NoCopy<u8>; 3], $behavior> = ArrayDeque::new();
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
                            let mut tester: ArrayDeque<[NoCopy<u8>; 3], $behavior> = ArrayDeque::new();
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
                            let mut tester: ArrayDeque<[NoCopy<u8>; 3], $behavior> = ArrayDeque::new();
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
            })
    }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_drain() {
        macro_rules! test {
            ($behavior:ident) => ({
        const CAP: usize = 7;
                let mut tester: ArrayDeque<[_; CAP + 1], $behavior> = ArrayDeque::new();

        for padding in 0..CAP {
            for drain_start in 0..CAP {
                for drain_end in drain_start..CAP {
                    // deque starts from different tail position
                    unsafe {
                        tester.set_head(padding);
                        tester.set_tail(padding);
                    }

                    tester.extend(0..CAP);

                    let mut expected = vec![0, 1, 2, 3, 4, 5, 6];
                    let drains: Vec<_> = tester.drain(drain_start..drain_end).collect();
                    let expected_drains: Vec<_> = expected.drain(drain_start..drain_end).collect();
                    assert_eq!(drains, expected_drains);
                    assert_eq!(tester.len(), expected.len());
                }
            }
        }
            })
    }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn saturating_drop() {
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
            let mut tester = ArrayDeque::<[Bump; 128], Saturating>::new();
            tester.push_back(Bump(flag));
            tester.push_back(Bump(flag));
        }
        assert_eq!(flag.get(), 2);

        // test something with the nullable pointer optimization
        flag.set(0);
        {
            let mut tester = ArrayDeque::<[_; 4], Saturating>::new();
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
    fn wrapping_drop() {
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
            let mut tester = ArrayDeque::<[Bump; 128], Wrapping>::new();
            tester.push_back(Bump(flag));
            tester.push_back(Bump(flag));
        }
        assert_eq!(flag.get(), 2);

        // test something with the nullable pointer optimization
        flag.set(0);
        {
            let mut tester = ArrayDeque::<[_; 4], Wrapping>::new();
            tester.push_back(vec![Bump(flag)]);
            tester.push_back(vec![Bump(flag), Bump(flag)]);
            tester.push_back(vec![]);
            tester.push_back(vec![Bump(flag)]);
            assert_eq!(flag.get(), 1);
            drop(tester.pop_back());
            assert_eq!(flag.get(), 2);
            drop(tester.pop_back());
            assert_eq!(flag.get(), 2);
        }
        assert_eq!(flag.get(), 4);
    }

    #[test]
    fn any_as_slice() {
        macro_rules! test {
            ($behavior:ident) => ({
        const CAP: usize = 10;
                let mut tester = ArrayDeque::<[_; CAP], $behavior>::new();

        for len in 0..CAP - 1 {
            for padding in 0..CAP {
                // deque starts from different tail position
                unsafe {
                    tester.set_head(padding);
                    tester.set_tail(padding);
                }

                let mut expected = vec![];
                tester.extend(0..len);
                expected.extend(0..len);

                let split_idx = CAP - padding;
                if split_idx < len {
                    assert_eq!(tester.as_slices(), expected[..].split_at(split_idx));
                } else {
                    assert_eq!(tester.as_slices(), (&expected[..], &[][..]));
                }
            }
        }
            })
    }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_partial_equal() {
        macro_rules! test {
            ($behavior:ident) => ({
        const CAP: usize = 10;
                let mut tester = ArrayDeque::<[f64; CAP], $behavior>::new();

        for len in 0..CAP - 1 {
            for padding in 0..CAP {
                // deque starts from different tail position
                unsafe {
                    tester.set_head(padding);
                    tester.set_tail(padding);
                }

                                let mut expected = ArrayDeque::<[f64; CAP], $behavior>::new();
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
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_fmt() {
        macro_rules! test {
            ($behavior:ident) => ({
                let mut tester = ArrayDeque::<[_; 5], $behavior>::new();
        tester.extend(0..4);
        assert_eq!(format!("{:?}", tester), "[0, 1, 2, 3]");
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn saturating_from_iterator() {
        let tester: ArrayDeque<[_; 5], Saturating>;
        let mut expected = ArrayDeque::<[_; 5], Saturating>::new();
        tester = ArrayDeque::from_iter(vec![0, 1, 2, 3, 4, 5]);
        expected.extend(0..4);
        assert_eq!(tester, expected);
    }

    #[test]
    fn wrapping_from_iterator() {
        let tester: ArrayDeque<[_; 5], Wrapping>;
        let mut expected = ArrayDeque::<[_; 5], Wrapping>::new();
        tester = vec![0, 1, 2, 3, 4, 5].into_iter().collect();
        expected.extend(2..6);
        assert_eq!(tester, expected);
    }

    #[test]
    fn saturating_extend() {
        let mut tester: ArrayDeque<[usize; 8], Saturating>;
        tester = vec![0, 1, 2, 3].into_iter().collect();
        tester.extend(vec![4, 5, 6, 7, 8, 9]);
        let expected = (0..8).collect::<ArrayDeque<[usize; 8], Saturating>>();
        assert_eq!(tester, expected);
    }

    #[test]
    fn wrapping_extend() {
        let mut tester: ArrayDeque<[usize; 8], Wrapping>;
        tester = vec![0, 1, 2, 3].into_iter().collect();
        tester.extend(vec![4, 5, 6, 7, 8, 9]);
        let expected = (3..10).collect::<ArrayDeque<[usize; 8], Wrapping>>();
        assert_eq!(tester, expected);
    }

    #[test]
    fn any_swap_front_back_remove() {
        macro_rules! test {
            ($behavior:ident) => ({
        fn test(back: bool) {
            const CAP: usize = 16;
                            let mut tester = ArrayDeque::<[_; CAP], $behavior>::new();
            let usable_cap = tester.capacity();
            let final_len = usable_cap / 2;

            for len in 0..final_len {
                        let expected: Vec<_> = if back {
                    (0..len).collect()
                } else {
                    (0..len).rev().collect()
                };
                for padding in 0..usable_cap {
                    unsafe {
                        tester.set_tail(padding);
                        tester.set_head(padding);
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
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_retain() {
        macro_rules! test {
            ($behavior:ident) => ({
                const CAP: usize = 11;
                let mut tester: ArrayDeque<[_; CAP], $behavior> = ArrayDeque::new();
        for padding in 0..CAP {
            unsafe {
                tester.set_tail(padding);
                tester.set_head(padding);
            }
            tester.extend(0..CAP);
            tester.retain(|x| x % 2 == 0);
            assert_eq!(tester.iter().count(), CAP / 2);
                }
            })
        }

        test!(Saturating);
        test!(Wrapping);
        }

    #[test]
    fn saturating_prepend() {
        let mut a: ArrayDeque<[_; 6], Saturating> = vec![5, 6, 7].into_iter().collect();
        let mut b: ArrayDeque<[_; 6], Saturating> = vec![1, 2, 3, 4].into_iter().collect();

        // normal prepend
        a.prepend(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [3, 4, 5, 6, 7]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // prepend nothing to something
        a.prepend(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [3, 4, 5, 6, 7]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // prepend something to nothing
        b.prepend(&mut a);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), [3, 4, 5, 6, 7]);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), []);
    }

    #[test]
    fn wrapping_extend_front() {
        let a = vec![9, 8, 7];
        let b = vec![6, 5, 4];
        let c = vec![3, 2, 1];

        let mut deque: ArrayDeque<[_; 8], Wrapping> = ArrayDeque::new();

        // extend empty deque, staying within capacity
        deque.extend_front(a);
        assert_eq!(deque, vec![7, 8, 9]);

        // extend non-empty deque, staying within capacity
        deque.extend_front(b);
        assert_eq!(deque, vec![4, 5, 6, 7, 8, 9]);

        // extend non-empty deque, exceeding capacity
        deque.extend_front(c);
        assert_eq!(deque, vec![1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn saturating_extend_front() {
        let a = vec![9, 8, 7];
        let b = vec![6, 5, 4];
        let c = vec![3, 2, 1];

        let mut deque: ArrayDeque<[_; 8], Saturating> = ArrayDeque::new();

        // extend empty deque, staying within capacity
        deque.extend_front(a);
        assert_eq!(deque, vec![7, 8, 9]);

        // extend non-empty deque, staying within capacity
        deque.extend_front(b);
        assert_eq!(deque, vec![4, 5, 6, 7, 8, 9]);

        // extend non-empty deque, exceeding capacity
        deque.extend_front(c);
        assert_eq!(deque, vec![3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn wrapping_extend_back() {
        let a = vec![1, 2, 3];
        let b = vec![4, 5, 6];
        let c = vec![7, 8, 9];

        let mut deque: ArrayDeque<[_; 8], Wrapping> = ArrayDeque::new();

        // extend empty deque, staying within capacity
        deque.extend_back(a);
        assert_eq!(deque, vec![1, 2, 3]);

        // extend non-empty deque, staying within capacity
        deque.extend_back(b);
        assert_eq!(deque, vec![1, 2, 3, 4, 5, 6]);

        // extend non-empty deque, exceeding capacity
        deque.extend_back(c);
        assert_eq!(deque, vec![3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn saturating_extend_back() {
        let a = vec![1, 2, 3];
        let b = vec![4, 5, 6];
        let c = vec![7, 8, 9];

        let mut deque: ArrayDeque<[_; 8], Saturating> = ArrayDeque::new();

        // extend empty deque, staying within capacity
        deque.extend_back(a);
        assert_eq!(deque, vec![1, 2, 3]);

        // extend non-empty deque, staying within capacity
        deque.extend_back(b);
        assert_eq!(deque, vec![1, 2, 3, 4, 5, 6]);

        // extend non-empty deque, exceeding capacity
        deque.extend_back(c);
        assert_eq!(deque, vec![1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn wrapping_prepend() {
        let mut a: ArrayDeque<[_; 6], Wrapping> = vec![5, 6, 7].into_iter().collect();
        let mut b: ArrayDeque<[_; 6], Wrapping> = vec![1, 2, 3, 4].into_iter().collect();

        // normal prepend
        a.prepend(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // prepend nothing to something
        a.prepend(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // prepend something to nothing
        b.prepend(&mut a);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), []);
    }

    #[test]
    fn saturating_append() {
        let mut a: ArrayDeque<[_; 6], Saturating> = vec![1, 2, 3].into_iter().collect();
        let mut b: ArrayDeque<[_; 6], Saturating> = vec![4, 5, 6, 7].into_iter().collect();

        // normal append
        a.append(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // append nothing to something
        a.append(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // append something to nothing
        b.append(&mut a);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), []);
    }

    #[test]
    fn wrapping_append() {
        let mut a: ArrayDeque<[_; 6], Wrapping> = vec![1, 2, 3].into_iter().collect();
        let mut b: ArrayDeque<[_; 6], Wrapping> = vec![4, 5, 6, 7].into_iter().collect();

        // normal append
        a.append(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [3, 4, 5, 6, 7]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // append nothing to something
        a.append(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [3, 4, 5, 6, 7]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // append something to nothing
        b.append(&mut a);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), [3, 4, 5, 6, 7]);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), []);
    }

    #[test]
    fn any_split_off() {
        macro_rules! test {
            ($behavior:ident) => ({
        const CAP: usize = 16;
                let mut tester = ArrayDeque::<[_; CAP], $behavior>::new();
        for len in 0..CAP {
            // index to split at
            for at in 0..len + 1 {
                for padding in 0..CAP {
                            let expected_self: Vec<_> = (0..).take(at).collect();
                            let expected_other: Vec<_> = (at..).take(len - at).collect();
                    unsafe {
                        tester.set_head(padding);
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
                            assert_eq!(tester, &expected_self[..]);
                            assert_eq!(result, &expected_other[..]);
                        }
                    }
                }
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn saturating_insert() {
        const CAP: usize = 16;
        let mut tester = ArrayDeque::<[_; CAP], Saturating>::new();

        // len is the length *after* insertion
        for len in 1..CAP {
            // 0, 1, 2, .., len - 1
            let expected: Vec<_> = (0..).take(len).collect();
            for padding in 0..CAP {
                for to_insert in 0..len {
                    unsafe {
                        tester.set_tail(padding);
                        tester.set_head(padding);
                    }
                    for i in 0..len {
                        if i != to_insert {
                            tester.push_back(i);
                        }
                    }
                    tester.insert(to_insert, to_insert);
                    assert!(tester.tail() < CAP);
                    assert!(tester.head() < CAP);
                    assert_eq!(tester, expected);
                }
            }
        }
    }

    #[test]
    fn wrapping_insert() {
        const CAP: usize = 16;
        let mut tester = ArrayDeque::<[_; CAP], Wrapping>::new();

        // len is the length *after* insertion
        for len in 1..CAP {
            // 0, 1, 2, .., len - 1
            let expected: Vec<_> = (0..).take(len).collect();
            for padding in 0..CAP {
                for to_insert in 0..len {
                    unsafe {
                        tester.set_tail(padding);
                        tester.set_head(padding);
                    }
                    for i in 0..len {
                        if i != to_insert {
                            tester.push_back(i);
                        }
                    }
                    tester.insert(to_insert, to_insert);
                    assert!(tester.tail() < CAP);
                    assert!(tester.head() < CAP);
                    assert_eq!(tester, expected);
                }
            }
        }
    }

    #[test]
    fn any_remove() {
        macro_rules! test {
            ($behavior:ident) => ({
        const CAP: usize = 16;
                let mut tester = ArrayDeque::<[_; CAP], $behavior>::new();

        // len is the length *after* removal
        for len in 0..CAP - 1 {
            // 0, 1, 2, .., len - 1
                    let expected: Vec<_> = (0..).take(len).collect();
            for padding in 0..CAP {
                for to_remove in 0..len + 1 {
                    unsafe {
                        tester.set_tail(padding);
                        tester.set_head(padding);
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
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }

    #[test]
    fn any_clone() {
        macro_rules! test {
            ($behavior:ident) => ({
                let tester: ArrayDeque<[_; 16], $behavior> = (0..16).into_iter().collect();
        let cloned = tester.clone();
        assert_eq!(tester, cloned)
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }
}

#[cfg(test)]
#[cfg(feature = "use_generic_array")]
mod test_generic_array {
    extern crate generic_array;

    use generic_array::GenericArray;
    use generic_array::typenum::U41;

    use super::*;

    #[test]
    fn any_simple() {
        macro_rules! test {
            ($behavior:ident) => ({
                let mut vec: ArrayDeque<GenericArray<i32, U41>, $behavior> = ArrayDeque::new();

                assert_eq!(vec.len(), 0);
                assert_eq!(vec.capacity(), 40);
                vec.extend(0..20);
                assert_eq!(vec.len(), 20);
                assert_eq!(vec.into_iter().take(5).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
            })
        }

        test!(Saturating);
        test!(Wrapping);
    }
}

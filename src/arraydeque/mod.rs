use std::ptr;

pub use odds::IndexRange as RangeArgument;
use nodrop::NoDrop;

use array::Array;
use array::Index as ArrayIndex;
use utils::*;

mod internal;
mod iterator_impls;
mod trait_impls;

/// `ArrayDeque` is a fixed capacity ring buffer.
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
pub struct ArrayDeque<A: Array> {
    xs: NoDrop<A>,
    head: A::Index,
    tail: A::Index,
}

/// `ArrayDeque` iterator
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct Iter<'a, T: 'a> {
    ring: &'a [T],
    head: usize,
    tail: usize,
}

/// `ArrayDeque` mutable iterator
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct IterMut<'a, T: 'a> {
    ring: &'a mut [T],
    head: usize,
    tail: usize,
}

/// A by-value `ArrayDeque` iterator
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct IntoIter<A: Array> {
    inner: ArrayDeque<A>,
}

/// A draining `ArrayDeque` iterator
pub struct Drain<'a, A>
    where A: Array,
          A::Item: 'a
{
    after_tail: usize,
    after_head: usize,
    iter: Iter<'a, A::Item>,
    deque: *mut ArrayDeque<A>,
}

impl<A: Array> ArrayDeque<A> {
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
    pub fn new() -> ArrayDeque<A> {
        unsafe {
            ArrayDeque {
                xs: NoDrop::new(super::new_array()),
                head: ArrayIndex::from(0),
                tail: ArrayIndex::from(0),
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
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// buf.push_back(3);
    /// buf.push_back(4);
    /// buf.push_back(5);
    /// assert_eq!(buf.get(1), Some(&4));
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
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// buf.push_back(3);
    /// buf.push_back(4);
    /// buf.push_back(5);
    /// if let Some(elem) = buf.get_mut(1) {
    ///     *elem = 7;
    /// }
    ///
    /// assert_eq!(buf[1], 7);
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
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// buf.push_back(3);
    /// buf.push_back(4);
    /// buf.push_back(5);
    /// buf.swap(0, 2);
    /// assert_eq!(buf[0], 5);
    /// assert_eq!(buf[2], 3);
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

    /// Return the capacity of the `ArrayVec`.
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
    ///
    /// let mut buf: ArrayDeque<[usize; 4]> = ArrayDeque::new();
    /// assert_eq!(buf.capacity(), 3);
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
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// buf.push_back(5);
    /// buf.push_back(3);
    /// buf.push_back(4);
    /// let b: &[_] = &[&5, &3, &4];
    /// let c: Vec<&i32> = buf.iter().collect();
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
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// buf.push_back(5);
    /// buf.push_back(3);
    /// buf.push_back(4);
    /// for num in buf.iter_mut() {
    ///     *num = *num - 2;
    /// }
    /// let b: &[_] = &[&mut 3, &mut 1, &mut 2];
    /// assert_eq!(&buf.iter_mut().collect::<Vec<&mut i32>>()[..], b);
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
    /// let mut v: ArrayDeque<[_; 4]> = vec![1, 2, 3].into_iter().collect();
    /// assert_eq!(vec![3].into_iter().collect::<ArrayDeque<[_; 4]>>(), v.drain(2..).collect());
    /// assert_eq!(vec![1, 2].into_iter().collect::<ArrayDeque<[_; 4]>>(), v);
    ///
    /// // A full range clears all contents
    /// v.drain(..);
    /// assert!(v.is_empty());
    /// ```
    pub fn drain<R>(&mut self, range: R) -> Drain<A>
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

    /// Provides a reference to the front element, or `None` if the sequence is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut d: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(d.front(), None);
    ///
    /// d.push_back(1);
    /// d.push_back(2);
    /// assert_eq!(d.front(), Some(&1));
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
    /// let mut d: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(d.front_mut(), None);
    ///
    /// d.push_back(1);
    /// d.push_back(2);
    /// match d.front_mut() {
    ///     Some(x) => *x = 9,
    ///     None => (),
    /// }
    /// assert_eq!(d.front(), Some(&9));
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
    /// let mut d: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(d.back(), None);
    ///
    /// d.push_back(1);
    /// d.push_back(2);
    /// assert_eq!(d.back(), Some(&2));
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
    /// let mut d: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(d.back(), None);
    ///
    /// d.push_back(1);
    /// d.push_back(2);
    /// match d.back_mut() {
    ///     Some(x) => *x = 9,
    ///     None => (),
    /// }
    /// assert_eq!(d.back(), Some(&9));
    /// ```
    pub fn back_mut(&mut self) -> Option<&mut A::Item> {
        let len = self.len();
        if !self.is_empty() {
            Some(&mut self[len - 1])
        } else {
            None
        }
    }

    /// Removes the first element and returns it, or `None` if the sequence is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut d: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// d.push_back(1);
    /// d.push_back(2);
    ///
    /// assert_eq!(d.pop_front(), Some(1));
    /// assert_eq!(d.pop_front(), Some(2));
    /// assert_eq!(d.pop_front(), None);
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

    /// Inserts an element first in the sequence.
    ///
    /// Return `None` if the push succeeds, or and return `Some(` *element* `)`
    /// if the vector is full.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut d: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// d.push_front(1);
    /// d.push_front(2);
    /// let overflow = d.push_front(3);
    ///
    /// assert_eq!(d.front(), Some(&2));
    /// assert_eq!(overflow, Some(3));
    /// ```
    pub fn push_front(&mut self, element: A::Item) -> Option<A::Item> {
        if !self.is_full() {
            unsafe {
                let new_tail = Self::wrap_sub(self.tail(), 1);
                self.set_tail(new_tail);
                self.buffer_write(new_tail, element);
            }
            None
        } else {
            Some(element)
        }
    }

    /// Appends an element to the back of a buffer
    ///
    /// Return `None` if the push succeeds, or and return `Some(` *element* `)`
    /// if the vector is full.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// buf.push_back(1);
    /// buf.push_back(3);
    /// let overflow = buf.push_back(5);
    ///
    /// assert_eq!(3, *buf.back().unwrap());
    /// assert_eq!(overflow, Some(5));
    /// ```
    pub fn push_back(&mut self, element: A::Item) -> Option<A::Item> {
        if !self.is_full() {
            unsafe {
                let head = self.head();
                self.set_head(Self::wrap_add(head, 1));
                self.buffer_write(head, element);
            }
            None
        } else {
            Some(element)
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
    /// let mut buf: ArrayDeque<[_; 3]> = ArrayDeque::new();
    /// assert_eq!(buf.pop_back(), None);
    /// buf.push_back(1);
    /// buf.push_back(3);
    /// assert_eq!(buf.pop_back(), Some(3));
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
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// assert_eq!(buf.swap_remove_back(0), None);
    /// buf.push_back(1);
    /// buf.push_back(2);
    /// buf.push_back(3);
    ///
    /// assert_eq!(buf.swap_remove_back(0), Some(1));
    /// assert_eq!(buf.len(), 2);
    /// assert_eq!(buf[0], 3);
    /// assert_eq!(buf[1], 2);
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
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// assert_eq!(buf.swap_remove_front(0), None);
    /// buf.push_back(1);
    /// buf.push_back(2);
    /// buf.push_back(3);
    ///
    /// assert_eq!(buf.swap_remove_front(2), Some(3));
    /// assert_eq!(buf.len(), 2);
    /// assert_eq!(buf[0], 2);
    /// assert_eq!(buf[1], 1);
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

    /// Inserts an element at `index` within the `ArrayDeque`. Whichever
    /// end is closer to the insertion point will be moved to make room,
    /// and all the affected elements will be moved to new positions.
    ///
    /// Return `None` if the push succeeds, or and return `Some(` *element* `)`
    /// if the vector is full.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than `ArrayDeque`'s length
    ///
    /// # Examples
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// buf.push_back(10);
    /// buf.push_back(12);
    /// buf.insert(1, 11);
    /// let overflow = buf.insert(0, 9);
    ///
    /// assert_eq!(Some(&11), buf.get(1));
    /// assert_eq!(overflow, Some(9));
    /// ```
    pub fn insert(&mut self, index: usize, element: A::Item) -> Option<A::Item> {
        assert!(index <= self.len(), "index out of bounds");
        if self.is_full() {
            return Some(element);
        }

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

        None
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
    /// let mut buf: ArrayDeque<[_; 4]> = ArrayDeque::new();
    /// buf.push_back(1);
    /// buf.push_back(2);
    /// buf.push_back(3);
    ///
    /// assert_eq!(buf.remove(1), Some(2));
    /// assert_eq!(buf.get(1), Some(&3));
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
    /// let mut buf: ArrayDeque<[_; 4]> = vec![1,2,3].into_iter().collect();
    /// let buf2 = buf.split_off(1);
    /// // buf = [1], buf2 = [2, 3]
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

    /// Moves all the elements of `other` into `Self`, leaving `other` empty.
    ///
    /// Does not extract more items than there is space for. No error
    /// occurs if there are more iterator elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use arraydeque::ArrayDeque;
    ///
    /// let mut buf: ArrayDeque<[_; 8]> = vec![1, 2, 3].into_iter().collect();
    /// // buf2: ArrayDeque<[_; 8]> can be infer
    /// let mut buf2 = vec![4, 5, 6].into_iter().collect();
    /// let mut buf3 = vec![7, 8, 9].into_iter().collect();
    ///
    /// buf.append(&mut buf2);
    /// assert_eq!(buf.len(), 6);
    /// assert_eq!(buf2.len(), 0);
    ///
    /// // max capacity reached
    /// buf.append(&mut buf3);
    /// assert_eq!(buf.len(), 7);
    /// assert_eq!(buf3.len(), 0);
    /// ```
    pub fn append(&mut self, other: &mut Self) {
        self.extend(other.drain(..));
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
    /// let mut buf: ArrayDeque<[_; 5]> = ArrayDeque::new();
    /// buf.extend(1..5);
    /// buf.retain(|&x| x%2 == 0);
    ///
    /// let v: Vec<_> = buf.into_iter().collect();
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
}

#[cfg(test)]
mod tests {
    use super::ArrayDeque;
    use std::vec::Vec;

    #[test]
    fn test_simple() {
        let mut tester: ArrayDeque<[_; 8]> = ArrayDeque::new();
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
    }

    #[test]
    fn test_simple_reversely() {
        let mut tester: ArrayDeque<[_; 8]> = ArrayDeque::new();
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
    }

    #[test]
    fn test_overflow() {
        let mut tester: ArrayDeque<[_; 3]> = ArrayDeque::new();
        assert_eq!(tester.push_back(1), None);
        assert_eq!(tester.push_back(2), None);
        assert_eq!(tester.push_back(3), Some(3));
    }

    #[test]
    fn test_pop_empty() {
        let mut tester: ArrayDeque<[_; 3]> = ArrayDeque::new();
        assert_eq!(tester.push_back(1), None);
        assert_eq!(tester.pop_front(), Some(1));
        assert_eq!(tester.is_empty(), true);
        assert_eq!(tester.len(), 0);
        assert_eq!(tester.pop_front(), None);
    }

    #[test]
    fn test_index() {
        let mut tester: ArrayDeque<[_; 4]> = ArrayDeque::new();
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
        let mut tester: ArrayDeque<[_; 4]> = ArrayDeque::new();
        tester.push_back(1);
        tester.push_back(2);
        tester[2];
    }

    #[test]
    fn test_iter() {
        let mut tester: ArrayDeque<[_; 3]> = ArrayDeque::new();
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
        let mut tester: ArrayDeque<[_; 3]> = ArrayDeque::new();
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
        const CAP: usize = 7;
        let mut tester: ArrayDeque<[_; CAP + 1]> = ArrayDeque::new();

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
            let mut tester = ArrayDeque::<[_; 4]>::new();
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
    }

    #[test]
    fn test_partial_equal() {
        const CAP: usize = 10;
        let mut tester = ArrayDeque::<[f64; CAP]>::new();

        for len in 0..CAP - 1 {
            for padding in 0..CAP {
                // deque starts from different tail position
                unsafe {
                    tester.set_head(padding);
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
        tester.extend(0..4);
        assert_eq!(format!("{:?}", tester), "[0, 1, 2, 3]");
    }

    #[test]
    fn test_from_iterator() {
        let tester: ArrayDeque<[_; 5]>;
        let mut expected = ArrayDeque::<[_; 5]>::new();
        tester = vec![0, 1, 2, 3].into_iter().collect();
        expected.extend(0..4);
        assert_eq!(tester, expected);
    }

    #[test]
    fn test_extend() {
        let mut tester: ArrayDeque<[usize; 10]>;
        tester = vec![0, 1, 2, 3].into_iter().collect();
        tester.extend(vec![4, 5, 6, 7]);
        let expected = (0..8).collect::<ArrayDeque<[usize; 10]>>();
        assert_eq!(tester, expected);
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
    }

    #[test]
    fn test_retain() {
        const CAP: usize = 10;
        let mut tester: ArrayDeque<[_; CAP]> = ArrayDeque::new();
        for padding in 0..CAP {
            unsafe {
                tester.set_tail(padding);
                tester.set_head(padding);
            }
            tester.extend(0..CAP);
            tester.retain(|x| x % 2 == 0);
            assert_eq!(tester.iter().count(), CAP / 2);
        }
    }

    #[test]
    fn test_append() {
        let mut a: ArrayDeque<[_; 16]> = vec![1, 2, 3].into_iter().collect();
        let mut b: ArrayDeque<[_; 16]> = vec![4, 5, 6].into_iter().collect();

        // normal append
        a.append(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [1, 2, 3, 4, 5, 6]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // append nothing to something
        a.append(&mut b);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), [1, 2, 3, 4, 5, 6]);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), []);

        // append something to nothing
        b.append(&mut a);
        assert_eq!(b.iter().cloned().collect::<Vec<_>>(), [1, 2, 3, 4, 5, 6]);
        assert_eq!(a.iter().cloned().collect::<Vec<_>>(), []);
    }

    #[test]
    fn test_split_off() {
        const CAP: usize = 16;
        let mut tester = ArrayDeque::<[_; CAP]>::new();
        for len in 0..CAP {
            // index to split at
            for at in 0..len + 1 {
                for padding in 0..CAP {
                    let expected_self = (0..).take(at).collect();
                    let expected_other = (at..).take(len - at).collect();
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
                    assert_eq!(tester, expected_self);
                    assert_eq!(result, expected_other);
                }
            }
        }
    }

    #[test]
    fn test_insert() {
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
    fn test_remove() {
        const CAP: usize = 16;
        let mut tester = ArrayDeque::<[_; CAP]>::new();

        // len is the length *after* removal
        for len in 0..CAP - 1 {
            // 0, 1, 2, .., len - 1
            let expected = (0..).take(len).collect();
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
    }

    #[test]
    fn test_clone() {
        let tester: ArrayDeque<[_; 16]> = (0..16).into_iter().collect();
        let cloned = tester.clone();
        assert_eq!(tester, cloned)
    }
}

#[cfg(test)]
#[cfg(feature = "use_generic_array")]
mod test_generic_array {
    extern crate generic_array;

    use generic_array::GenericArray;
    use generic_array::typenum::U41;

    use super::ArrayDeque;

    #[test]
    fn test_simple() {
        let mut vec: ArrayDeque<GenericArray<i32, U41>> = ArrayDeque::new();

        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 40);
        vec.extend(0..20);
        assert_eq!(vec.len(), 20);
        assert_eq!(vec.into_iter().take(5).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
    }
}

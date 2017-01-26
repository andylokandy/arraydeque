use std::cmp;
use std::ptr;
use std::slice;

use super::RotatingDeque;
use ::array::Array;
use ::array::Index as ArrayIndex;
use utils::*;

impl<A: Array> RotatingDeque<A> {
    #[inline]
    pub fn wrap_add(index: usize, addend: usize) -> usize {
        wrap_add(index, addend, A::capacity())
    }

    #[inline]
    pub fn wrap_sub(index: usize, subtrahend: usize) -> usize {
        wrap_sub(index, subtrahend, A::capacity())
    }

    #[inline]
    pub fn ptr(&self) -> *const A::Item {
        self.xs.as_ptr()
    }

    #[inline]
    pub fn ptr_mut(&mut self) -> *mut A::Item {
        self.xs.as_mut_ptr()
    }

    #[inline]
    pub fn is_contiguous(&self) -> bool {
        self.tail() <= self.head()
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        A::capacity() - self.len() == 1
    }

    #[inline]
    pub fn head(&self) -> usize {
        self.head.to_usize()
    }

    #[inline]
    pub fn tail(&self) -> usize {
        self.tail.to_usize()
    }

    #[inline]
    pub unsafe fn set_head(&mut self, head: usize) {
        debug_assert!(head <= self.capacity());
        self.head = ArrayIndex::from(head);
    }

    #[inline]
    pub unsafe fn set_tail(&mut self, tail: usize) {
        debug_assert!(tail <= self.capacity());
        self.tail = ArrayIndex::from(tail);
    }

    /// Copies a contiguous block of memory len long from src to dst
    #[inline]
    pub unsafe fn copy(&mut self, dst: usize, src: usize, len: usize) {
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
    pub unsafe fn wrap_copy(&mut self, dst: usize, src: usize, len: usize) {
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
    pub unsafe fn buffer_as_slice(&self) -> &[A::Item] {
        slice::from_raw_parts(self.ptr(), A::capacity())
    }

    #[inline]
    pub unsafe fn buffer_as_mut_slice(&mut self) -> &mut [A::Item] {
        slice::from_raw_parts_mut(self.ptr_mut(), A::capacity())
    }

    #[inline]
    pub unsafe fn buffer_read(&mut self, offset: usize) -> A::Item {
        ptr::read(self.ptr().offset(offset as isize))
    }

    #[inline]
    pub unsafe fn buffer_replace(&mut self, offset: usize, element: A::Item) -> A::Item {
        ptr::replace(self.ptr_mut().offset(offset as isize), element)
    }

    #[inline]
    pub unsafe fn buffer_write(&mut self, offset: usize, element: A::Item) {
        ptr::write(self.ptr_mut().offset(offset as isize), element);
    }
}

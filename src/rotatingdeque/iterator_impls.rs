use super::*;

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

impl<A: Array> Iterator for IntoIter<A> {
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

impl<A: Array> DoubleEndedIterator for IntoIter<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A::Item> {
        self.inner.pop_back()
    }
}

impl<A: Array> ExactSizeIterator for IntoIter<A> {}

impl<'a, A> Drop for Drain<'a, A>
    where A: Array,
          A::Item: 'a
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
                    let new_tail = RotatingDeque::<A>::wrap_sub(drain_head, tail_len);
                    source_deque.set_tail(new_tail);
                    source_deque.wrap_copy(new_tail, orig_tail, tail_len);
                } else {
                    let new_head = RotatingDeque::<A>::wrap_add(drain_tail, head_len);
                    source_deque.set_head(new_head);
                    source_deque.wrap_copy(drain_tail, drain_head, head_len);
                }
            },
        }
    }
}

impl<'a, A> Iterator for Drain<'a, A>
    where A: Array,
          A::Item: 'a
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

impl<'a, A> DoubleEndedIterator for Drain<'a, A>
    where A: Array,
          A::Item: 'a
{
    #[inline]
    fn next_back(&mut self) -> Option<A::Item> {
        self.iter.next_back().map(|elt| unsafe { ptr::read(elt) })
    }
}

impl<'a, A> ExactSizeIterator for Drain<'a, A>
    where A: Array,
          A::Item: 'a
{
}

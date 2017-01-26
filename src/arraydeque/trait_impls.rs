use std::cmp::Ordering;
use std::fmt;
use std::iter;
use std::hash::{Hash, Hasher};
use std::ops::Index;
use std::ops::IndexMut;

use super::ArrayDeque;
use super::*;
use ::array::Array;

impl<A: Array> Clone for ArrayDeque<A>
    where A::Item: Clone
{
    fn clone(&self) -> ArrayDeque<A> {
        self.iter().cloned().collect()
    }
}

impl<A: Array> Drop for ArrayDeque<A> {
    fn drop(&mut self) {
        self.clear();

        // NoDrop inhibits array's drop
        // panic safety: NoDrop::drop will trigger on panic, so the inner
        // array will not drop even after panic.
    }
}

impl<A: Array> Default for ArrayDeque<A> {
    #[inline]
    fn default() -> ArrayDeque<A> {
        ArrayDeque::new()
    }
}

impl<A: Array> PartialEq for ArrayDeque<A>
    where A::Item: PartialEq
{
    fn eq(&self, other: &ArrayDeque<A>) -> bool {
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

impl<A: Array> Eq for ArrayDeque<A> where A::Item: Eq {}

impl<A: Array> PartialOrd for ArrayDeque<A>
    where A::Item: PartialOrd
{
    fn partial_cmp(&self, other: &ArrayDeque<A>) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<A: Array> Ord for ArrayDeque<A>
    where A::Item: Ord
{
    #[inline]
    fn cmp(&self, other: &ArrayDeque<A>) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<A: Array> Hash for ArrayDeque<A>
    where A::Item: Hash
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        let (a, b) = self.as_slices();
        Hash::hash_slice(a, state);
        Hash::hash_slice(b, state);
    }
}

impl<A: Array> Index<usize> for ArrayDeque<A> {
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

impl<A: Array> IndexMut<usize> for ArrayDeque<A> {
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

impl<A: Array> iter::FromIterator<A::Item> for ArrayDeque<A> {
    fn from_iter<T: IntoIterator<Item = A::Item>>(iter: T) -> Self {
        let mut array = ArrayDeque::new();
        array.extend(iter);
        array
    }
}

impl<A: Array> IntoIterator for ArrayDeque<A> {
    type Item = A::Item;
    type IntoIter = IntoIter<A>;

    fn into_iter(self) -> IntoIter<A> {
        IntoIter { inner: self }
    }
}

impl<'a, A: Array> IntoIterator for &'a ArrayDeque<A> {
    type Item = &'a A::Item;
    type IntoIter = Iter<'a, A::Item>;

    fn into_iter(self) -> Iter<'a, A::Item> {
        self.iter()
    }
}

impl<'a, A: Array> IntoIterator for &'a mut ArrayDeque<A> {
    type Item = &'a mut A::Item;
    type IntoIter = IterMut<'a, A::Item>;

    fn into_iter(mut self) -> IterMut<'a, A::Item> {
        self.iter_mut()
    }
}

/// Extend the `ArrayDeque` with an iterator.
///
/// Does not extract more items than there is space for. No error
/// occurs if there are more iterator elements.
impl<A: Array> Extend<A::Item> for ArrayDeque<A> {
    fn extend<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
        let take = self.capacity() - self.len();
        for elt in iter.into_iter().take(take) {
            self.push_back(elt);
        }
    }
}

impl<A: Array> fmt::Debug for ArrayDeque<A>
    where A::Item: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self).finish()
    }
}


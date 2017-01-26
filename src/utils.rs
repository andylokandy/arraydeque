#[inline]
pub fn wrap_add(index: usize, addend: usize, capacity: usize) -> usize {
    debug_assert!(addend <= capacity);
    (index + addend) % capacity
}

#[inline]
pub fn wrap_sub(index: usize, subtrahend: usize, capacity: usize) -> usize {
    debug_assert!(subtrahend <= capacity);
    (index + capacity - subtrahend) % capacity
}

#[inline]
pub fn count(tail: usize, head: usize, capacity: usize) -> usize {
    debug_assert!(head < capacity);
    debug_assert!(tail < capacity);
    if head >= tail {
        head - tail
    } else {
        capacity + head - tail
    }
}

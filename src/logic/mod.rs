pub mod insert;
pub mod remove;
pub mod copy;

pub trait CircularBuffer {
    fn array_len(&self) -> usize;

    fn head(&self) -> usize;
    fn tail(&self) -> usize;

    unsafe fn set_head(&mut self, head: usize);
    unsafe fn set_tail(&mut self, tail: usize);

    unsafe fn copy(&mut self, dst: usize, src: usize, len: usize);

    fn wrap_add(index: usize, addend: usize) -> usize;
    fn wrap_sub(index: usize, subtrahend: usize) -> usize;
}

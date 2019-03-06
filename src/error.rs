#[cfg(feature = "std")]
use std::error::Error;
use std::fmt;

/// Error value indicating insufficient capacity
///
/// This error only occur to `ArrayDeque<_, Saturating>`.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct CapacityError<T = ()> {
    /// The element that caused the error.
    pub element: T,
}

const CAPERROR: &'static str = "insufficient capacity";

#[cfg(feature = "std")]
impl<T> Error for CapacityError<T> {
    fn description(&self) -> &str {
        CAPERROR
    }
}

impl<T> fmt::Display for CapacityError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", CAPERROR)
    }
}

impl<T> fmt::Debug for CapacityError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", "CapacityError", CAPERROR)
    }
}

//! Behavior semantics for `ArrayDeque`.

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
pub struct Wrapping;
impl Behavior for Wrapping {}

/// Behavior for `ArrayDeque` that specifies saturating write semantics.
///
/// ### Pushing to back:
///
/// Pushing elements to the **back** of a fixed-size deque that **has already reached its capacity**
/// causes it **exit early, without performing any mutation**.
///
/// ### Pushing to front:
///
/// Pushing elements to the **front** of a fixed-size deque that **has already reached its capacity**
/// causes it **exit early, without performing any mutation**.
pub struct Saturating;
impl Behavior for Saturating {}

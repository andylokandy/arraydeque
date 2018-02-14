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
/// # Examples
///
/// ```text
/// [_, _, _] + 1 => [_, _, 1] -> None
/// [_, _, 1] + 2 => [_, 1, 2] -> None
/// [_, 1, 2] + 3 => [1, 2, 3] -> None
/// [1, 2, 3] + 4 => [2, 3, 4] -> Some(1)
/// ```
///
/// ### Pushing to front:
///
/// Pushing elements to the **front** of a fixed-size deque that **has already reached its capacity**
/// causes it to **overwrite** existing elements from the **back**.
///
/// # Examples
///
/// ```text
/// 1 + [_, _, _] => [1, _, _] -> None
/// 2 + [1, _, _] => [2, 1, _] -> None
/// 3 + [2, 1, _] => [3, 2, 1] -> None
/// 4 + [3, 2, 1] => [4, 3, 2] -> Some(1)
/// ```
pub struct Wrapping;
impl Behavior for Wrapping {}

/// Behavior for `ArrayDeque` that specifies saturating write semantics.
///
/// ### Pushing to back:
///
/// Pushing elements to the **back** of a fixed-size deque that **has already reached its capacity**
/// causes it **exit early, without performing any mutation**.
///
/// # Examples
///
/// ```text
/// [_, _, _] + 1 => [_, _, 1] -> None
/// [_, _, 1] + 2 => [_, 1, 2] -> None
/// [_, 1, 2] + 3 => [1, 2, 3] -> None
/// [1, 2, 3] + 4 => [1, 2, 3] -> Some(4)
/// ```
///
/// ### Pushing to front:
///
/// Pushing elements to the **front** of a fixed-size deque that **has already reached its capacity**
/// causes it **exit early, without performing any mutation**.
///
/// # Examples
///
/// ```text
/// 1 + [_, _, _] => [1, _, _] -> None
/// 2 + [1, _, _] => [2, 1, _] -> None
/// 3 + [2, 1, _] => [3, 2, 1] -> None
/// 4 + [3, 2, 1] => [3, 2, 1] -> Some(4)
/// ```
pub struct Saturating;
impl Behavior for Saturating {}

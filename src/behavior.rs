//! Behavior semantics for `ArrayDeque`.

/// Marker trait for indicating behaviors of `ArrayDeque`.
pub trait Behavior {}

/// Behavior for `ArrayDeque` that specifies saturating write semantics.
pub struct Saturating;

impl Behavior for Saturating {}

/// Behavior for `ArrayDeque` that specifies wrapping write semantics.
pub struct Wrapping;

impl Behavior for Wrapping {}

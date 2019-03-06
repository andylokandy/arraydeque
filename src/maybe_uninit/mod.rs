#[cfg(has_manually_drop_in_union)]
mod maybe_uninit;
#[cfg(not(has_manually_drop_in_union))]
#[path = "maybe_uninit_nodrop.rs"]
mod maybe_uninit;
#[cfg(not(has_manually_drop_in_union))]
mod nodrop;

pub use self::maybe_uninit::MaybeUninit;

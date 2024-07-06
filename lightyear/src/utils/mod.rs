//! Contains a set of useful utilities

pub(crate) mod free_list;

pub(crate) mod ready_buffer;

pub(crate) mod sequence_buffer;

pub mod bevy;

#[cfg_attr(docsrs, doc(cfg(feature = "avian2d")))]
#[cfg(feature = "avian2d")]
pub mod avian2d;

pub(crate) mod captures;
pub(crate) mod pool;
pub mod wrapping_id;

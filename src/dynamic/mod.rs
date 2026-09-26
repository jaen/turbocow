//! A clone-on-write byte vector that stores short vectors inline.

mod common;
mod types;

pub(crate) use common::limit::LIMIT;
pub(crate) use types::*;

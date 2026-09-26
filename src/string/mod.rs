//! A clone-on-write, small-string-optimized alternative to [`String`].

mod default;

pub use default::{EcoStr, EcoString, ToEcoString};

#[cfg(feature = "rkyv")]
mod rkyv_impl;

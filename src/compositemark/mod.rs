//! Port of `src/compositemark/`.
//!
//! Composite marks (`boxplot`, `errorbar`, `errorband`) are expanded during
//! normalization into layers of primitive marks.

pub mod base;
pub mod boxplot;
pub mod common;
pub mod errorband;
pub mod errorbar;
pub mod index;

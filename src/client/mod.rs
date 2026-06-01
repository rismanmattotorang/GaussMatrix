//! # gauss-core — GaussInteract shared client core
//!
//! `gauss-core` is the single shared Rust core of the GaussInteract client
//! ([`GaussMatrix-SPECS.pdf`], §V): synchronisation, the local store, and E2EE
//! (delegated to `vodozemac`) are implemented once and exposed to one Flutter
//! presentation layer across four targets.
//!
//! This module provides the **simplified sliding-sync window** (MSC4186, §V-C):
//! the client materialises only the *visible window* of rooms and lazily
//! expands it, which is what makes the `< 1.2 s` cold-start target attainable on
//! mid-range mobile — the heavy paths never touch rooms outside the window.
//!
//! [`GaussMatrix-SPECS.pdf`]: ../../GaussMatrix-SPECS.pdf

#![forbid(unsafe_code)]

mod sync;
#[cfg(test)]
mod tests;

pub use self::sync::SlidingWindow;

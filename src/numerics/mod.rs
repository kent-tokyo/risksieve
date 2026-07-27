//! Numerical routines shared across controllers.
//!
//! AGENTS.md section 8 requires numerical correctness alongside
//! statistical correctness. This module holds the shared, auditable
//! primitives that section relies on, rather than duplicating them in
//! every controller.

pub mod summation;

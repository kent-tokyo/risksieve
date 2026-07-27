//! Distribution-shift support: importance-weight validation and
//! diagnostics.
//!
//! AGENTS.md section 3: distribution-shift support is an explicit
//! extension, never an implicit default. Nothing in this crate learns a
//! density ratio from raw features (AGENTS.md's out-of-scope list); a
//! caller supplies importance weights already computed by their own
//! method, and this module only validates them and tracks the summary
//! statistics a shifted controller needs.

pub mod importance;

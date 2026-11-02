//! `tpt-reficon` — facade crate.
//!
//! Re-exports the macro frontend and the runtime types behind one `prelude`
//! so application code can write:
//!
//! ```
//! use tpt_reficon::prelude::*;
//!
//! #[derive(Refine)]
//! #[requires(value > 0 && value <= 100)]
//! pub struct BoundedPercentage(u8);
//! ```
//!
//! Crates available directly if you prefer explicit paths:
//! [`tpt-reficon-macros`](https://docs.rs/tpt-reficon-macros) and
//! [`tpt-reficon-core`](https://docs.rs/tpt-reficon-core). See the workspace
//! [`README`](https://github.com/tpt-solutions/tpt-reficon#readme) for the
//! project pitch and runnable examples.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Re-export of the runtime crate so macro-generated code can resolve
/// `tpt_reficon::core::*` regardless of dependency renames.
pub use tpt_reficon_core as core;

/// One-import front door: `use tpt_reficon::prelude::*;` brings in the derive
/// macro and the runtime types.
pub mod prelude {
    /// The `Refine` trait (type namespace).
    #[doc(hidden)]
    pub use tpt_reficon_core::Refine;
    /// Runtime-check error type.
    pub use tpt_reficon_core::RefinementError;
    /// The `#[contract]` attribute for pre- and postconditions on functions.
    pub use tpt_reficon_macros::contract;
    /// The `#[ffi_contract]` attribute (needs the `ffi` feature to expand).
    pub use tpt_reficon_macros::ffi_contract;
    /// Safe wrappers around C functions, checked against a header (needs the `ffi` feature).
    pub use tpt_reficon_macros::ffi_wrappers;
    /// The `#[derive(Refine)]` proc macro (macro namespace).
    #[doc(hidden)]
    pub use tpt_reficon_macros::Refine;
}

pub use prelude::*;

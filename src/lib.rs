//! A [containerd] shim that runs WebAssembly containers on the [zwasm] runtime.
//!
//! The crate builds on [`containerd_shim_wasm`] and provides [`ZwasmShim`], a
//! [`Shim`](containerd_shim_wasm::shim::Shim) implementation whose sandbox
//! executes a container's WebAssembly module through [`zwasm_sdk`]. The
//! `containerd-shim-zwasm-v1` binary shipped with this crate is what containerd
//! invokes for the `io.containerd.zwasm.v1` runtime.
//!
//! # Usage
//!
//! The binary is a thin wrapper around the shim:
//!
//! ```no_run
//! use containerd_shim_wasm::shim::Cli;
//! use containerd_shim_zwasm::ZwasmShim;
//!
//! ZwasmShim::run(None);
//! ```
//!
//! # Requirements
//!
//! Building this crate compiles the zwasm C library from source, which requires
//! [Zig] 0.16.0 or later in `PATH`. See the crate's README for how to register
//! the shim with containerd.
//!
//! [containerd]: https://containerd.io
//! [zwasm]: https://github.com/clojurewasm/zwasm
//! [Zig]: https://ziglang.org

pub mod instance;

pub use instance::{ZwasmSandbox, ZwasmShim};

#[cfg(unix)]
#[cfg(test)]
#[path = "tests.rs"]
mod zwasm_tests;

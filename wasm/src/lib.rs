//! Raw WebAssembly exports for SafeImage layer 1.
//!
//! No `wasm-bindgen`, no generated glue: the module has **no imports**, so a host instantiates
//! it synchronously from an already-compiled module (`new WebAssembly.Instance(module, {})`)
//! and calls the exports below. Memory is the module's own linear memory; the host allocates
//! with [`si_alloc`], writes the RGB bytes, calls [`si_pdq`] or [`si_pdq_dihedral`], reads the
//! output buffer, and frees with [`si_free`].
//!
//! Output layout, for `si_pdq`: 33 bytes = 32-byte hash (words 0..15, each little-endian, see
//! `Hash256::to_bytes`) followed by one quality byte. For `si_pdq_dihedral`: 8 hashes of 32
//! bytes in the order original, rotate 90, rotate 180, rotate 270, flip X, flip Y, flip plus 1,
//! flip minus 1, followed by one quality byte (257 bytes).
//!
//! Errors are returned as negative `i32` codes; a panic would abort the module (a trap) and the
//! functions are written so that no input reaches one.

pub mod codec;
pub mod evidence;
pub mod evidence_codec;
pub mod report_codec;

mod abi;
pub use abi::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_evidence;

#[cfg(test)]
mod tests_report;

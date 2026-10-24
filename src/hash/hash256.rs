// SPDX-License-Identifier: BSD-3-Clause
// The bit layout and the text form follow facebook/ThreatExchange pdq/cpp/common/pdqhashtypes.h
// and pdqhashtypes.cpp at commit 07b82cb6e87b7e0ac7fc2a01d865df5db10ee1f2
// (blobs 02051b96eb2d5f38be282a1d97c5acb1a9d48cae and 7e6114c709f78f14829645edee429e8495483ed4).
// Original: Copyright (c) Meta Platforms, Inc. and affiliates.
// Licence: see LICENSE in this directory.
//
// Upstream stores 16 words of 16 bits. Bit `k` lives in word `k >> 4` at position `k & 15`, and the
// text form is 64 lowercase hex digits with word 15 first. Hamming distance is the population
// count of the XOR (upstream does this through a lookup table; the arithmetic is the same).

use alloc::string::String;
use core::fmt;

/// A 256-bit PDQ hash.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Hash256 {
    w: [u16; 16],
}

/// Why a hex string is not a [`Hash256`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HexError {
    /// The text is not exactly 64 characters.
    Length {
        /// Number of bytes in the text that was given.
        actual: usize,
    },
    /// A character is not a hexadecimal digit.
    Digit {
        /// Byte offset of the offending character.
        index: usize,
    },
}

impl fmt::Display for HexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HexError::Length { actual } => {
                write!(f, "hash text must be 64 hex digits, got {actual} bytes")
            }
            HexError::Digit { index } => write!(f, "non-hex character at byte {index}"),
        }
    }
}

impl Hash256 {
    /// Number of bits in the hash.
    pub const BITS: usize = 256;

    /// The all-zero hash (what upstream returns for a frame too small to hash).
    pub const fn zero() -> Self {
        Hash256 { w: [0; 16] }
    }

    /// Builds a hash from upstream's 16 words (`w[0]` holds bits 0 to 15).
    pub const fn from_words(w: [u16; 16]) -> Self {
        Hash256 { w }
    }

    /// The 16 words (`w[0]` holds bits 0 to 15).
    pub const fn words(&self) -> [u16; 16] {
        self.w
    }

    /// The 32-byte form: words `w[0]` to `w[15]` in order, each little-endian. This is the layout
    /// the WASM binding writes; it is **not** the text order of [`Hash256::to_hex`].
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, word) in self.w.iter().enumerate() {
            out[i * 2..i * 2 + 2].copy_from_slice(&word.to_le_bytes());
        }
        out
    }

    /// Bit `k` (taken modulo 256, as upstream does).
    pub fn get_bit(&self, k: usize) -> bool {
        (self.w[(k & 255) >> 4] >> (k & 15)) & 1 == 1
    }

    pub(crate) fn set_bit(&mut self, k: usize) {
        self.w[(k & 255) >> 4] |= 1 << (k & 15);
    }

    /// Number of differing bits between two hashes.
    pub fn hamming_distance(&self, other: &Hash256) -> u32 {
        self.w
            .iter()
            .zip(other.w.iter())
            .map(|(a, b)| (a ^ b).count_ones())
            .sum()
    }

    /// Upstream's text form: 64 lowercase hex digits, word 15 first.
    pub fn to_hex(&self) -> String {
        use core::fmt::Write;
        let mut s = String::with_capacity(64);
        for word in self.w.iter().rev() {
            // Writing to a String cannot fail.
            let _ = write!(s, "{word:04x}");
        }
        s
    }

    /// Parses upstream's text form (either case).
    pub fn from_hex(text: &str) -> Result<Self, HexError> {
        let bytes = text.as_bytes();
        if bytes.len() != 64 {
            return Err(HexError::Length {
                actual: bytes.len(),
            });
        }
        let mut w = [0u16; 16];
        let (chunks, _) = bytes.as_chunks::<4>();
        for (n, chunk) in chunks.iter().enumerate() {
            let mut v = 0u16;
            for (m, &c) in chunk.iter().enumerate() {
                let d = match c {
                    b'0'..=b'9' => c - b'0',
                    b'a'..=b'f' => c - b'a' + 10,
                    b'A'..=b'F' => c - b'A' + 10,
                    _ => return Err(HexError::Digit { index: n * 4 + m }),
                };
                v = (v << 4) | u16::from(d);
            }
            w[15 - n] = v;
        }
        Ok(Hash256 { w })
    }
}

impl fmt::Debug for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash256({})", self.to_hex())
    }
}

impl fmt::Display for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

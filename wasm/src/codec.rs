// SPDX-License-Identifier: MIT OR Apache-2.0
//! Versioned binary encoding for the structured types that cross the boundary (layer 2 so far).
//!
//! Hand-rolled, no dependencies (`docs/WASM-BOUNDARY.md` section 3). Every message starts with
//! [`VERSION`]. Integers are little-endian. A string is a `u32` byte length then UTF-8. An option
//! is a `0`/`1` byte then the value. An enum is a tag byte then its payload. A sequence is a `u32`
//! count then the items.
//!
//! This module is safe Rust and never panics on input: every failure is a [`CodecError`]. Decoding
//! never trusts a count to size an allocation: items are pushed as they are read, so a hostile
//! count costs at most the bytes actually present.
//!
//! Unknown tags fail closed: an unknown outcome tag or an unknown unavailable reason decodes to
//! `Unavailable(MalformedResponse)`, never to `Clean`; an unknown classification or match type
//! tag is [`CodecError::UnknownTag`], because the value cannot be preserved.

use safeimage::hash::Hash256;
use safeimage::matcher::{
    Classification, CleanBasis, Digest, KnownMatch, MatchDetail, MatchOutcome, MatchType,
    Unavailable,
};

/// Encoding version, the first byte of every message.
pub const VERSION: u8 = 1;

/// Why a buffer could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// The first byte is not [`VERSION`].
    BadVersion,
    /// The buffer ended inside a value.
    Truncated,
    /// A tag byte this module does not know, where the value cannot be preserved.
    UnknownTag,
    /// A string was not valid UTF-8, or a flag byte was neither 0 nor 1.
    Malformed,
    /// Bytes remained after the value.
    TrailingBytes,
    /// Two actor ids that must differ were equal (the two-person rule).
    SamePerson,
}

/// Writes primitives into a growing buffer.
#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// A writer that has already written [`VERSION`].
    pub fn new() -> Self {
        Writer { buf: vec![VERSION] }
    }

    /// The bytes written.
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }

    /// One byte.
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Four bytes, little-endian.
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Eight bytes, little-endian.
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Length-prefixed UTF-8.
    pub fn str(&mut self, s: &str) {
        // A string longer than u32::MAX cannot exist in 32-bit module memory.
        self.u32(s.len() as u32);
        self.buf.extend_from_slice(s.as_bytes());
    }
}

/// Reads primitives from a slice.
pub struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    /// Checks the version byte and returns a reader over the remainder.
    pub fn new(bytes: &'a [u8]) -> Result<Self, CodecError> {
        match bytes.split_first() {
            None => Err(CodecError::Truncated),
            Some((&v, rest)) if v == VERSION => Ok(Reader { rest }),
            Some(_) => Err(CodecError::BadVersion),
        }
    }

    /// Succeeds only when every byte was consumed.
    pub fn end(&self) -> Result<(), CodecError> {
        if self.rest.is_empty() {
            Ok(())
        } else {
            Err(CodecError::TrailingBytes)
        }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        if self.rest.len() < n {
            return Err(CodecError::Truncated);
        }
        let (head, tail) = self.rest.split_at(n);
        self.rest = tail;
        Ok(head)
    }

    /// One byte.
    pub fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }

    /// Four bytes, little-endian.
    pub fn u32(&mut self) -> Result<u32, CodecError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Eight bytes, little-endian.
    pub fn u64(&mut self) -> Result<u64, CodecError> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }

    /// Length-prefixed UTF-8.
    pub fn str(&mut self) -> Result<String, CodecError> {
        let n = self.u32()? as usize;
        let b = self.take(n)?;
        core::str::from_utf8(b)
            .map(String::from)
            .map_err(|_| CodecError::Malformed)
    }

    /// A `0`/`1` flag.
    pub fn flag(&mut self) -> Result<bool, CodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CodecError::Malformed),
        }
    }

    /// Exactly `n` raw bytes.
    pub fn raw(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        self.take(n)
    }
}

// Outcome tags.
const OUT_CLEAN: u8 = 0;
const OUT_UNAVAILABLE: u8 = 1;
const OUT_MATCH: u8 = 2;

/// Wire tag of an [`Unavailable`] reason. Part of the ABI; also returned by `si_classify_status`.
pub fn unavailable_tag(u: Unavailable) -> u8 {
    match u {
        Unavailable::NotConfigured => 0,
        Unavailable::Disabled => 1,
        Unavailable::Timeout => 2,
        Unavailable::ServerError => 3,
        Unavailable::Throttled => 4,
        Unavailable::CredentialsRefused => 5,
        Unavailable::MalformedResponse => 6,
        Unavailable::TestValueInProduction => 7,
    }
}

/// Reason for a wire tag. An unknown tag is `MalformedResponse`: still unavailable, never clean.
pub fn unavailable_from_tag(t: u8) -> Unavailable {
    match t {
        0 => Unavailable::NotConfigured,
        1 => Unavailable::Disabled,
        2 => Unavailable::Timeout,
        3 => Unavailable::ServerError,
        4 => Unavailable::Throttled,
        5 => Unavailable::CredentialsRefused,
        7 => Unavailable::TestValueInProduction,
        _ => Unavailable::MalformedResponse,
    }
}

fn put_opt_str(w: &mut Writer, s: &Option<String>) {
    match s {
        None => w.u8(0),
        Some(s) => {
            w.u8(1);
            w.str(s);
        }
    }
}

fn get_opt_str(r: &mut Reader<'_>) -> Result<Option<String>, CodecError> {
    if r.flag()? {
        Ok(Some(r.str()?))
    } else {
        Ok(None)
    }
}

fn put_outcome(w: &mut Writer, o: &MatchOutcome) {
    match o {
        MatchOutcome::Clean(b) => {
            w.u8(OUT_CLEAN);
            put_opt_str(w, &b.list_version);
            match b.checked_at {
                None => w.u8(0),
                Some(t) => {
                    w.u8(1);
                    w.u64(t);
                }
            }
        }
        MatchOutcome::Unavailable(u) => {
            w.u8(OUT_UNAVAILABLE);
            w.u8(unavailable_tag(*u));
        }
        MatchOutcome::Match(m) => {
            w.u8(OUT_MATCH);
            match &m.classification {
                Classification::Other(s) => {
                    w.u8(0);
                    w.str(s);
                }
                Classification::HarmfulAbusive => w.u8(1),
                Classification::Csam => w.u8(2),
            }
            w.u8(match m.match_type {
                MatchType::Near => 0,
                MatchType::Exact => 1,
            });
            w.u32(m.details.len() as u32);
            for d in &m.details {
                w.u32(d.digests.len() as u32);
                for g in &d.digests {
                    w.str(&g.algorithm);
                    w.str(&g.value);
                }
                put_opt_str(w, &d.listed_at);
            }
        }
    }
}

fn get_outcome(r: &mut Reader<'_>) -> Result<MatchOutcome, CodecError> {
    match r.u8()? {
        OUT_CLEAN => {
            let list_version = get_opt_str(r)?;
            let checked_at = if r.flag()? { Some(r.u64()?) } else { None };
            Ok(MatchOutcome::Clean(CleanBasis {
                list_version,
                checked_at,
            }))
        }
        OUT_UNAVAILABLE => Ok(MatchOutcome::Unavailable(unavailable_from_tag(r.u8()?))),
        OUT_MATCH => {
            let classification = match r.u8()? {
                0 => Classification::Other(r.str()?),
                1 => Classification::HarmfulAbusive,
                2 => Classification::Csam,
                _ => return Err(CodecError::UnknownTag),
            };
            let match_type = match r.u8()? {
                0 => MatchType::Near,
                1 => MatchType::Exact,
                _ => return Err(CodecError::UnknownTag),
            };
            let n_details = r.u32()?;
            let mut details = Vec::new();
            for _ in 0..n_details {
                let n_digests = r.u32()?;
                let mut digests = Vec::new();
                for _ in 0..n_digests {
                    digests.push(Digest {
                        algorithm: r.str()?,
                        value: r.str()?,
                    });
                }
                details.push(MatchDetail {
                    digests,
                    listed_at: get_opt_str(r)?,
                });
            }
            // Not `KnownMatch::new`: re-capping here would silently drop what the host sent.
            // The inputs are bounded by the module's input cap instead.
            Ok(MatchOutcome::Match(KnownMatch {
                classification,
                match_type,
                details,
            }))
        }
        // Fails closed: unavailable, never clean (docs/WASM-BOUNDARY.md rule 2).
        _ => Ok(MatchOutcome::Unavailable(Unavailable::MalformedResponse)),
    }
}

/// Encodes one outcome as a complete message.
pub fn encode_outcome(o: &MatchOutcome) -> Vec<u8> {
    let mut w = Writer::new();
    put_outcome(&mut w, o);
    w.finish()
}

/// Decodes one outcome from a complete message.
#[cfg(test)]
pub fn decode_outcome(bytes: &[u8]) -> Result<MatchOutcome, CodecError> {
    let mut r = Reader::new(bytes)?;
    let o = get_outcome(&mut r)?;
    r.end()?;
    Ok(o)
}

/// Encodes a list of outcomes (the input of `si_strongest`).
#[cfg(test)]
pub fn encode_outcomes(list: &[MatchOutcome]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u32(list.len() as u32);
    for o in list {
        put_outcome(&mut w, o);
    }
    w.finish()
}

/// Decodes a list of outcomes.
pub fn decode_outcomes(bytes: &[u8]) -> Result<Vec<MatchOutcome>, CodecError> {
    let mut r = Reader::new(bytes)?;
    let n = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(get_outcome(&mut r)?);
    }
    r.end()?;
    Ok(out)
}

/// Decodes a hash list: a `u32` count, then that many 32-byte hashes in the layer 1 byte layout.
pub fn decode_hashes(bytes: &[u8]) -> Result<Vec<Hash256>, CodecError> {
    let mut r = Reader::new(bytes)?;
    let n = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..n {
        let b = r.raw(32)?;
        let mut w = [0u16; 16];
        for (i, word) in w.iter_mut().enumerate() {
            *word = u16::from_le_bytes([b[i * 2], b[i * 2 + 1]]);
        }
        out.push(Hash256::from_words(w));
    }
    r.end()?;
    Ok(out)
}

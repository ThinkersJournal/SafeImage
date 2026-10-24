// SPDX-License-Identifier: MIT OR Apache-2.0
//! Encoding of the layer 3 types that cross the boundary: evidence states and events, the case
//! record, the two-person pair, a clear approval and the preservation clocks. Same conventions as
//! [`crate::codec`].
//!
//! Fail closed: every unknown tag is [`CodecError::UnknownTag`] (the host holds, nothing changes).
//! An identity that [`ActorId::new`] refuses (blank, or over 128 bytes) is
//! [`CodecError::Malformed`], and two equal identities are [`CodecError::SamePerson`]: the
//! two-person rule is checked by construction on decode, exactly as in the Rust API.

use safeimage::evidence::{
    ActorId, CaseKind, CaseRecord, ClearApproval, Destruction, DestructionBasis, EvidenceEvent,
    EvidenceState, FileRecord, LegalHold, ReportClock, ReviewOutcome, TwoPeople,
};

use crate::codec::{CodecError, Reader, Writer};
use crate::report_codec::{status_from_tag, status_tag};

/// Wire tag of an [`EvidenceState`]. Part of the ABI.
pub fn state_tag(s: EvidenceState) -> u8 {
    match s {
        EvidenceState::Pending => 0,
        EvidenceState::Present => 1,
        EvidenceState::Failed => 2,
        EvidenceState::Missing => 3,
        EvidenceState::FileWithoutEvidence => 4,
    }
}

/// State for a wire tag, or an unknown-tag error.
pub fn state_from_tag(t: u8) -> Result<EvidenceState, CodecError> {
    Ok(match t {
        0 => EvidenceState::Pending,
        1 => EvidenceState::Present,
        2 => EvidenceState::Failed,
        3 => EvidenceState::Missing,
        4 => EvidenceState::FileWithoutEvidence,
        _ => return Err(CodecError::UnknownTag),
    })
}

fn put_actor(w: &mut Writer, a: &ActorId) {
    w.str(a.as_str());
}

fn get_actor(r: &mut Reader<'_>) -> Result<ActorId, CodecError> {
    ActorId::new(r.str()?).ok_or(CodecError::Malformed)
}

fn put_two(w: &mut Writer, t: &TwoPeople) {
    put_actor(w, t.first());
    put_actor(w, t.second());
}

fn get_two(r: &mut Reader<'_>) -> Result<TwoPeople, CodecError> {
    let a = get_actor(r)?;
    let b = get_actor(r)?;
    TwoPeople::new(a, b).map_err(|_| CodecError::SamePerson)
}

/// Decodes a two-person pair from a complete message.
pub fn decode_two(bytes: &[u8]) -> Result<TwoPeople, CodecError> {
    let mut r = Reader::new(bytes)?;
    let t = get_two(&mut r)?;
    r.end()?;
    Ok(t)
}

/// Encodes a pair as a complete message.
pub fn encode_two(t: &TwoPeople) -> Vec<u8> {
    let mut w = Writer::new();
    put_two(&mut w, t);
    w.finish()
}

/// Decodes an evidence event from a complete message.
pub fn decode_event(bytes: &[u8]) -> Result<EvidenceEvent, CodecError> {
    let mut r = Reader::new(bytes)?;
    let e = match r.u8()? {
        0 => EvidenceEvent::PutSucceeded,
        1 => EvidenceEvent::PutFailed,
        2 => EvidenceEvent::ObjectMissing,
        3 => EvidenceEvent::DecidedWithoutEvidence(get_two(&mut r)?),
        _ => return Err(CodecError::UnknownTag),
    };
    r.end()?;
    Ok(e)
}

/// Encodes an evidence event as a complete message.
pub fn encode_event(e: &EvidenceEvent) -> Vec<u8> {
    let mut w = Writer::new();
    match e {
        EvidenceEvent::PutSucceeded => w.u8(0),
        EvidenceEvent::PutFailed => w.u8(1),
        EvidenceEvent::ObjectMissing => w.u8(2),
        EvidenceEvent::DecidedWithoutEvidence(t) => {
            w.u8(3);
            put_two(&mut w, t);
        }
    }
    w.finish()
}

/// Decodes a clear approval: tag 0 one actor, tag 1 two different actors.
pub fn decode_approval(bytes: &[u8]) -> Result<ClearApproval, CodecError> {
    let mut r = Reader::new(bytes)?;
    let a = match r.u8()? {
        0 => ClearApproval::One(get_actor(&mut r)?),
        1 => ClearApproval::Two(get_two(&mut r)?),
        _ => return Err(CodecError::UnknownTag),
    };
    r.end()?;
    Ok(a)
}

/// Encodes a clear approval.
pub fn encode_approval(a: &ClearApproval) -> Vec<u8> {
    let mut w = Writer::new();
    match a {
        ClearApproval::One(x) => {
            w.u8(0);
            put_actor(&mut w, x);
        }
        ClearApproval::Two(t) => {
            w.u8(1);
            put_two(&mut w, t);
        }
    }
    w.finish()
}

/// Decodes a list of evidence states: a `u32` count then one tag byte each.
pub fn decode_states(bytes: &[u8]) -> Result<Vec<EvidenceState>, CodecError> {
    let mut r = Reader::new(bytes)?;
    let n = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(state_from_tag(r.u8()?)?);
    }
    r.end()?;
    Ok(out)
}

/// Decodes a list of `(state, object_found)`: a `u32` count then a tag byte and a flag each.
pub fn decode_state_found(bytes: &[u8]) -> Result<Vec<(EvidenceState, bool)>, CodecError> {
    let mut r = Reader::new(bytes)?;
    let n = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..n {
        let s = state_from_tag(r.u8()?)?;
        out.push((s, r.flag()?));
    }
    r.end()?;
    Ok(out)
}

/// Decodes report clocks: a `u32` count, then per report a report status tag, a `u32` count of
/// send times (`u64` each) and an optional finish time.
pub fn decode_clocks(bytes: &[u8]) -> Result<Vec<ReportClock>, CodecError> {
    let mut r = Reader::new(bytes)?;
    let n = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..n {
        let status = status_from_tag(r.u8()?)?;
        let n_sent = r.u32()?;
        let mut sent_at = Vec::new();
        for _ in 0..n_sent {
            sent_at.push(r.u64()?);
        }
        let finished_at = if r.flag()? { Some(r.u64()?) } else { None };
        out.push(ReportClock {
            status,
            sent_at,
            finished_at,
        });
    }
    r.end()?;
    Ok(out)
}

/// Encodes report clocks (the inverse of [`decode_clocks`]; used by the vector generator).
pub fn encode_clocks(list: &[ReportClock]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u32(list.len() as u32);
    for c in list {
        w.u8(status_tag(c.status));
        w.u32(c.sent_at.len() as u32);
        for t in &c.sent_at {
            w.u64(*t);
        }
        match c.finished_at {
            None => w.u8(0),
            Some(t) => {
                w.u8(1);
                w.u64(t);
            }
        }
    }
    w.finish()
}

fn kind_tag(k: CaseKind) -> u8 {
    match k {
        CaseKind::KnownHash => 0,
        CaseKind::Classifier => 1,
        CaseKind::RemovalRequest => 2,
        CaseKind::ModeratorSighting => 3,
    }
}

fn kind_from_tag(t: u8) -> Result<CaseKind, CodecError> {
    Ok(match t {
        0 => CaseKind::KnownHash,
        1 => CaseKind::Classifier,
        2 => CaseKind::RemovalRequest,
        3 => CaseKind::ModeratorSighting,
        _ => return Err(CodecError::UnknownTag),
    })
}

/// Encodes a case record as a complete message.
pub fn encode_case(c: &CaseRecord) -> Vec<u8> {
    let mut w = Writer::new();
    w.u8(kind_tag(c.kind));
    w.u64(c.created_at);
    match c.review {
        None => w.u8(0),
        Some((o, at)) => {
            w.u8(1);
            w.u8(match o {
                ReviewOutcome::Confirmed => 0,
                ReviewOutcome::FalsePositive => 1,
            });
            w.u64(at);
        }
    }
    w.u32(c.files.len() as u32);
    for f in &c.files {
        w.str(&f.sha256_hex);
        w.u8(state_tag(f.state));
    }
    match &c.evidence_hold {
        None => w.u8(0),
        Some(h) => {
            w.u8(1);
            put_two(&mut w, &h.set_by);
            w.str(&h.reason);
            w.u64(h.set_at);
        }
    }
    match &c.destruction {
        None => w.u8(0),
        Some(d) => {
            w.u8(1);
            match &d.basis {
                DestructionBasis::LawEnforcementRequest { request_ref } => {
                    w.u8(0);
                    w.str(request_ref);
                }
                DestructionBasis::ClearedAndExpired => w.u8(1),
                // A basis added by a newer core than this table: the host must alarm.
                _ => w.u8(crate::report_codec::UNKNOWN_VARIANT),
            }
            w.u64(d.at);
            match &d.by {
                None => w.u8(0),
                Some(t) => {
                    w.u8(1);
                    put_two(&mut w, t);
                }
            }
        }
    }
    w.finish()
}

/// Decodes a case record from a complete message.
pub fn decode_case(bytes: &[u8]) -> Result<CaseRecord, CodecError> {
    let mut r = Reader::new(bytes)?;
    let kind = kind_from_tag(r.u8()?)?;
    let created_at = r.u64()?;
    let review = if r.flag()? {
        let o = match r.u8()? {
            0 => ReviewOutcome::Confirmed,
            1 => ReviewOutcome::FalsePositive,
            _ => return Err(CodecError::UnknownTag),
        };
        Some((o, r.u64()?))
    } else {
        None
    };
    let n_files = r.u32()?;
    let mut files = Vec::new();
    for _ in 0..n_files {
        let sha256_hex = r.str()?;
        files.push(FileRecord {
            sha256_hex,
            state: state_from_tag(r.u8()?)?,
        });
    }
    let evidence_hold = if r.flag()? {
        let set_by = get_two(&mut r)?;
        let reason = r.str()?;
        Some(LegalHold {
            set_by,
            reason,
            set_at: r.u64()?,
        })
    } else {
        None
    };
    let destruction = if r.flag()? {
        let basis = match r.u8()? {
            0 => DestructionBasis::LawEnforcementRequest {
                request_ref: r.str()?,
            },
            1 => DestructionBasis::ClearedAndExpired,
            _ => return Err(CodecError::UnknownTag),
        };
        let at = r.u64()?;
        let by = if r.flag()? {
            Some(get_two(&mut r)?)
        } else {
            None
        };
        Some(Destruction { basis, at, by })
    } else {
        None
    };
    r.end()?;
    Ok(CaseRecord {
        kind,
        created_at,
        review,
        files,
        evidence_hold,
        destruction,
    })
}

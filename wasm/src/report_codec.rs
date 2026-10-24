// SPDX-License-Identifier: MIT OR Apache-2.0
//! Encoding of the layer 4 types that cross the boundary: the persisted `ReportState`, a `Step`,
//! an `Event` and a `DestinationAnswer`. Same conventions as [`crate::codec`].
//!
//! Fail closed: an unknown tag in a **state** or a **step** is [`CodecError::UnknownTag`] (the
//! host holds the report; nothing is sent), and an unknown **answer** tag decodes to
//! `DestinationAnswer::Unknown`, which is handled as transient and never as success.

use safeimage::report::{
    DestinationAnswer, Event, FileProgress, FinishNote, HoldReason, ReportState, Status, Step,
};

use crate::codec::{CodecError, Reader, Writer};

/// Wire tag written for a variant of a `non_exhaustive` enum that this module does not know
/// (only possible if the core crate is newer than this table). A host treats it as an alarm.
pub const UNKNOWN_VARIANT: u8 = 255;

pub(crate) fn status_tag(s: Status) -> u8 {
    match s {
        Status::AwaitingCredentials => 0,
        Status::Pending => 1,
        Status::Submitted => 2,
        Status::Finished => 3,
        Status::Failed => 4,
        Status::RetractPending => 5,
        Status::Withdrawn => 6,
    }
}

pub(crate) fn status_from_tag(t: u8) -> Result<Status, CodecError> {
    Ok(match t {
        0 => Status::AwaitingCredentials,
        1 => Status::Pending,
        2 => Status::Submitted,
        3 => Status::Finished,
        4 => Status::Failed,
        5 => Status::RetractPending,
        6 => Status::Withdrawn,
        _ => return Err(CodecError::UnknownTag),
    })
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

fn put_state_body(w: &mut Writer, s: &ReportState) {
    w.u8(status_tag(s.status));
    w.u32(s.attempts);
    match s.next_attempt_at {
        None => w.u8(0),
        Some(t) => {
            w.u8(1);
            w.u64(t);
        }
    }
    put_opt_str(w, &s.report_id);
    w.u32(s.files.len() as u32);
    for f in &s.files {
        w.u8(u8::from(f.uploaded));
        w.u8(u8::from(f.described));
    }
    put_opt_str(w, &s.last_code);
    w.u32(s.abandoned_ids.len() as u32);
    for id in &s.abandoned_ids {
        w.str(id);
    }
    match s.finish_note {
        None => w.u8(0),
        Some(FinishNote::AlreadyFinished) => w.u8(1),
        Some(FinishNote::FinishedBeforeRetract) => w.u8(2),
        Some(_) => w.u8(UNKNOWN_VARIANT),
    }
}

fn get_state_body(r: &mut Reader<'_>) -> Result<ReportState, CodecError> {
    let status = status_from_tag(r.u8()?)?;
    let attempts = r.u32()?;
    let next_attempt_at = if r.flag()? { Some(r.u64()?) } else { None };
    let report_id = get_opt_str(r)?;
    let n_files = r.u32()?;
    let mut files = Vec::new();
    for _ in 0..n_files {
        files.push(FileProgress {
            uploaded: r.flag()?,
            described: r.flag()?,
        });
    }
    let last_code = get_opt_str(r)?;
    let n_ids = r.u32()?;
    let mut abandoned_ids = Vec::new();
    for _ in 0..n_ids {
        abandoned_ids.push(r.str()?);
    }
    let finish_note = match r.u8()? {
        0 => None,
        1 => Some(FinishNote::AlreadyFinished),
        2 => Some(FinishNote::FinishedBeforeRetract),
        _ => return Err(CodecError::UnknownTag),
    };
    Ok(ReportState {
        status,
        attempts,
        next_attempt_at,
        report_id,
        files,
        last_code,
        abandoned_ids,
        finish_note,
    })
}

/// Encodes a state as a complete message.
pub fn encode_state(s: &ReportState) -> Vec<u8> {
    let mut w = Writer::new();
    put_state_body(&mut w, s);
    w.finish()
}

/// Decodes a state from a complete message.
pub fn decode_state(bytes: &[u8]) -> Result<ReportState, CodecError> {
    let mut r = Reader::new(bytes)?;
    let s = get_state_body(&mut r)?;
    r.end()?;
    Ok(s)
}

fn put_event(w: &mut Writer, e: &Event) {
    match e {
        Event::AuthRefused => w.u8(0),
        Event::Invalid(d) => {
            w.u8(1);
            w.str(d);
        }
        Event::WindowExpired => w.u8(2),
        Event::UnknownAnswer(c) => {
            w.u8(3);
            w.str(c);
        }
        Event::FinishedBeforeRetract => w.u8(4),
        Event::MissingReportId => w.u8(5),
        Event::StepNotValidHere => w.u8(6),
        _ => w.u8(UNKNOWN_VARIANT),
    }
}

/// Encodes the result of `apply`: the new state, then an optional event, as one message.
pub fn encode_applied(s: &ReportState, e: &Option<Event>) -> Vec<u8> {
    let mut w = Writer::new();
    put_state_body(&mut w, s);
    match e {
        None => w.u8(0),
        Some(e) => {
            w.u8(1);
            put_event(&mut w, e);
        }
    }
    w.finish()
}

/// Encodes a step as a complete message.
pub fn encode_step(s: &Step) -> Vec<u8> {
    let mut w = Writer::new();
    match s {
        Step::Submit => w.u8(0),
        Step::Upload { file } => {
            w.u8(1);
            w.u32(*file as u32);
        }
        Step::Describe { file } => {
            w.u8(2);
            w.u32(*file as u32);
        }
        Step::Finish => w.u8(3),
        Step::Retract => w.u8(4),
        Step::Wait { until } => {
            w.u8(5);
            w.u64(*until);
        }
        Step::Hold(HoldReason::AwaitingCredentials) => {
            w.u8(6);
            w.u8(0);
        }
        Step::Hold(HoldReason::Failed) => {
            w.u8(6);
            w.u8(1);
        }
        Step::Done => w.u8(7),
    }
    w.finish()
}

/// Decodes a step from a complete message.
pub fn decode_step(bytes: &[u8]) -> Result<Step, CodecError> {
    let mut r = Reader::new(bytes)?;
    let s = match r.u8()? {
        0 => Step::Submit,
        1 => Step::Upload {
            file: r.u32()? as usize,
        },
        2 => Step::Describe {
            file: r.u32()? as usize,
        },
        3 => Step::Finish,
        4 => Step::Retract,
        5 => Step::Wait { until: r.u64()? },
        6 => match r.u8()? {
            0 => Step::Hold(HoldReason::AwaitingCredentials),
            1 => Step::Hold(HoldReason::Failed),
            _ => return Err(CodecError::UnknownTag),
        },
        7 => Step::Done,
        _ => return Err(CodecError::UnknownTag),
    };
    r.end()?;
    Ok(s)
}

/// Encodes an answer as a complete message.
pub fn encode_answer(a: &DestinationAnswer) -> Vec<u8> {
    let mut w = Writer::new();
    match a {
        DestinationAnswer::Accepted { report_id } => {
            w.u8(0);
            put_opt_str(&mut w, report_id);
        }
        DestinationAnswer::Transient => w.u8(1),
        DestinationAnswer::AuthRefused => w.u8(2),
        DestinationAnswer::Invalid(d) => {
            w.u8(3);
            w.str(d);
        }
        DestinationAnswer::AlreadyFinished => w.u8(4),
        DestinationAnswer::WindowExpired => w.u8(5),
        DestinationAnswer::Unknown(c) => {
            w.u8(6);
            w.str(c);
        }
    }
    w.finish()
}

/// Decodes an answer. An unknown tag is `Unknown("unknown_tag")`: transient, never success.
pub fn decode_answer(bytes: &[u8]) -> Result<DestinationAnswer, CodecError> {
    let mut r = Reader::new(bytes)?;
    let a = match r.u8()? {
        0 => DestinationAnswer::Accepted {
            report_id: get_opt_str(&mut r)?,
        },
        1 => DestinationAnswer::Transient,
        2 => DestinationAnswer::AuthRefused,
        3 => DestinationAnswer::Invalid(r.str()?),
        4 => DestinationAnswer::AlreadyFinished,
        5 => DestinationAnswer::WindowExpired,
        6 => DestinationAnswer::Unknown(r.str()?),
        // The rest of the message is not read: its shape is unknown. Handled as transient.
        _ => return Ok(DestinationAnswer::Unknown("unknown_tag".into())),
    };
    r.end()?;
    Ok(a)
}

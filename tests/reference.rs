//! Bit-for-bit agreement with the upstream PDQ reference at the pinned commit.
//!
//! `tests/fixtures/reference.txt` is produced by `tests/reference/build.sh`, which builds the
//! unmodified upstream sources and a harness. Frames are the synthetic ones in `support`.

mod support;

use safeimage::hash::{__reference_support as rs, Frame, Hash256, pdq, pdq_dihedral};
use std::collections::HashMap;

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(1099511628211);
    }
    h
}

fn fnv_f32(v: &[f32]) -> u64 {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for x in v {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    fnv(&bytes)
}

#[derive(Default)]
struct Ref {
    luma_hashing: u64,
    luma_downscaling: u64,
    pdq: Option<(String, i32)>,
    dihedral_quality: i32,
    dihedral: Vec<String>,
}

fn parse() -> (String, HashMap<String, Ref>) {
    let text = include_str!("fixtures/reference.txt");
    let mut dct_line = String::new();
    let mut map: HashMap<String, Ref> = HashMap::new();
    let mut cur = String::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        match f[0] {
            "dct64to16" => dct_line = line.to_string(),
            "torben" => {} // read by `torben_matches_the_reference`
            "frame" => {
                cur = f[1].to_string();
                map.insert(cur.clone(), Ref::default());
            }
            "luma_hashing" => {
                map.get_mut(&cur).unwrap().luma_hashing = u64::from_str_radix(f[1], 16).unwrap()
            }
            "luma_downscaling" => {
                map.get_mut(&cur).unwrap().luma_downscaling = u64::from_str_radix(f[1], 16).unwrap()
            }
            "pdq" => {
                map.get_mut(&cur).unwrap().pdq = Some((f[1].to_string(), f[2].parse().unwrap()))
            }
            "dihedral_quality" => {
                map.get_mut(&cur).unwrap().dihedral_quality = f[1].parse().unwrap()
            }
            "dihedral" => map.get_mut(&cur).unwrap().dihedral.push(f[2].to_string()),
            other => panic!("unknown fixture line kind {other}"),
        }
    }
    (dct_line, map)
}

#[test]
fn fixture_covers_every_synthetic_frame() {
    let (_, map) = parse();
    assert_eq!(map.len(), support::FRAMES.len());
    for (name, ..) in support::FRAMES {
        assert!(map.contains_key(*name), "fixture lacks {name}");
    }
}

#[test]
fn both_upstream_luma_definitions_agree_with_each_other_and_with_ours() {
    let (_, map) = parse();
    for (name, w, h, pattern) in support::FRAMES {
        let r = &map[*name];
        assert_eq!(
            r.luma_hashing, r.luma_downscaling,
            "{name}: upstream definitions differ"
        );
        let rgb = support::make(*w, *h, *pattern);
        let frame = Frame::new(&rgb, *w, *h).unwrap();
        assert_eq!(
            fnv_f32(&rs::luma(&frame)),
            r.luma_hashing,
            "{name}: luma differs"
        );
    }
}

#[test]
fn single_hash_and_quality_match_the_reference_exactly() {
    let (_, map) = parse();
    for (name, w, h, pattern) in support::FRAMES {
        let rgb = support::make(*w, *h, *pattern);
        let frame = Frame::new(&rgb, *w, *h).unwrap();
        let got = pdq(&frame);
        let (hex, q) = map[*name].pdq.clone().unwrap();
        assert_eq!(got.hash.to_hex(), hex, "{name}: hash");
        assert_eq!(i32::from(got.quality), q, "{name}: quality");
    }
}

#[test]
fn all_eight_dihedral_hashes_match_the_reference_exactly() {
    let (_, map) = parse();
    for (name, w, h, pattern) in support::FRAMES {
        let rgb = support::make(*w, *h, *pattern);
        let frame = Frame::new(&rgb, *w, *h).unwrap();
        let got = pdq_dihedral(&frame);
        let r = &map[*name];
        assert_eq!(
            i32::from(got.quality),
            r.dihedral_quality,
            "{name}: quality"
        );
        assert_eq!(r.dihedral.len(), 8, "{name}: fixture has 8 hashes");
        for (i, hash) in got.hashes.iter().enumerate() {
            assert_eq!(hash.to_hex(), r.dihedral[i], "{name}: dihedral {i}");
        }
    }
}

#[test]
fn dct_stage_matches_the_reference_bit_for_bit() {
    let (line, _) = parse();
    let f: Vec<&str> = line.split(' ').collect();
    let out = rs::dct64_to_16(&support::dct_input());
    let mut bytes = Vec::new();
    for x in &out {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    assert_eq!(format!("{:016x}", fnv(&bytes)), f[1]);
    for k in 0..4 {
        assert_eq!(format!("{:08x}", out[k].to_bits()), f[2 + k], "B[0][{k}]");
    }
}

#[test]
fn hex_round_trip_is_the_upstream_text_form() {
    let (_, map) = parse();
    let hex = &map["sq512"].pdq.clone().unwrap().0;
    let h = Hash256::from_hex(hex).unwrap();
    assert_eq!(&h.to_hex(), hex);
    assert_eq!(h.hamming_distance(&h), 0);
}

#[test]
fn torben_matches_the_reference() {
    let text = include_str!("fixtures/reference.txt");
    let mut seen = 0;
    for line in text.lines().filter(|l| l.starts_with("torben ")) {
        let f: Vec<&str> = line.split(' ').collect();
        let (_, values) = support::torben_cases()
            .into_iter()
            .find(|(n, _)| *n == f[1])
            .unwrap();
        assert_eq!(
            format!("{:08x}", rs::torben(&values).to_bits()),
            f[2],
            "case {}",
            f[1]
        );
        seen += 1;
    }
    assert_eq!(seen, support::torben_cases().len());
}

// SPDX-License-Identifier: LicenseRef-PublicDomain
// The following code is public domain.
// Algorithm by Torben Mogensen, implementation by N. Devillard.
// This code in public domain.
//
// Ported from facebook/ThreatExchange pdq/cpp/hashing/torben.cpp at commit
// 07b82cb6e87b7e0ac7fc2a01d865df5db10ee1f2 (blob ae1d059596b9b6f367f647fe1899ee8bd9ae1142).
// The upstream file carries the public-domain statement above and no other copyright line;
// this port carries the same statement and no Meta line.

/// Median of `m` by Torben's method (upstream `torben`), on floats, with the upstream's exact
/// comparisons so that ties resolve the same way. `m` must be non-empty.
pub(crate) fn torben(m: &[f32]) -> f32 {
    let n = m.len() as i32;
    let mut min = m[0];
    let mut max = m[0];
    for &v in &m[1..] {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }

    let mut guess;
    let mut less;
    let mut equal;
    let mut maxltguess;
    let mut mingtguess;
    loop {
        guess = (min + max) / 2.0;
        less = 0;
        let mut greater = 0;
        equal = 0;
        maxltguess = min;
        mingtguess = max;
        for &v in m {
            if v < guess {
                less += 1;
                if v > maxltguess {
                    maxltguess = v;
                }
            } else if v > guess {
                greater += 1;
                if v < mingtguess {
                    mingtguess = v;
                }
            } else {
                equal += 1;
            }
        }
        if less <= (n + 1) / 2 && greater <= (n + 1) / 2 {
            break;
        } else if less > greater {
            max = maxltguess;
        } else {
            min = mingtguess;
        }
    }
    if less >= (n + 1) / 2 {
        maxltguess
    } else if less + equal >= (n + 1) / 2 {
        guess
    } else {
        mingtguess
    }
}

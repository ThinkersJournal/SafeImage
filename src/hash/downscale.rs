// Derived from facebook/ThreatExchange pdq/cpp/downscaling/downscaling.cpp at commit
// 07b82cb6e87b7e0ac7fc2a01d865df5db10ee1f2 (blob 03a62e3bbc8a1b4d53bfa0a9616e4a117c11a7ab).
// Original: Copyright (c) Meta Platforms, Inc. and affiliates.
// Licence: see LICENSE in this directory.
//
// Functions keep the upstream names (snake-cased) so a reviewer can diff them by eye:
// computeJaroszFilterWindowSize, jaroszFilterFloat, boxAlongRowsFloat, boxAlongColsFloat,
// box1DFloat, decimateFloat.

/// Upstream `computeJaroszFilterWindowSize`.
pub(crate) fn compute_jarosz_filter_window_size(
    old_dimension: usize,
    new_dimension: usize,
) -> usize {
    old_dimension.div_ceil(2 * new_dimension)
}

/// Upstream `jaroszFilterFloat`: `nreps` rounds of a row pass (buffer1 to buffer2) then a column
/// pass (buffer2 to buffer1). The result is left in `buffer1`.
pub(crate) fn jarosz_filter_float(
    buffer1: &mut [f32],
    buffer2: &mut [f32],
    num_rows: usize,
    num_cols: usize,
    window_size_along_rows: usize,
    window_size_along_cols: usize,
    nreps: usize,
) {
    for _ in 0..nreps {
        box_along_rows_float(buffer1, buffer2, num_rows, num_cols, window_size_along_rows);
        box_along_cols_float(buffer2, buffer1, num_rows, num_cols, window_size_along_cols);
    }
}

/// Upstream `box1DFloat`. `invec` and `outvec` are indexed with `stride`, starting at their
/// first element.
fn box1d_float(
    invec: &[f32],
    outvec: &mut [f32],
    vector_length: usize,
    stride: usize,
    full_window_size: usize,
) {
    let half_window_size = (full_window_size + 2) / 2; // 7->4, 8->5

    let phase_1_nreps = half_window_size - 1;
    let phase_2_nreps = full_window_size - half_window_size + 1;
    let phase_3_nreps = vector_length - full_window_size;
    let phase_4_nreps = half_window_size - 1;

    let mut li = 0usize; // left edge of the read window, for subtracts
    let mut ri = 0usize; // right edge of the read window, for adds
    let mut oi = 0usize; // index into the output vector

    let mut sum = 0.0f32;
    let mut current_window_size = 0u32;

    // PHASE 1: ACCUMULATE FIRST SUM NO WRITES
    for _ in 0..phase_1_nreps {
        sum += invec[ri];
        current_window_size += 1;
        ri += stride;
    }

    // PHASE 2: INITIAL WRITES WITH SMALL WINDOW
    for _ in 0..phase_2_nreps {
        sum += invec[ri];
        current_window_size += 1;
        outvec[oi] = sum / current_window_size as f32;
        ri += stride;
        oi += stride;
    }

    // PHASE 3: WRITES WITH FULL WINDOW
    for _ in 0..phase_3_nreps {
        sum += invec[ri];
        sum -= invec[li];
        outvec[oi] = sum / current_window_size as f32;
        li += stride;
        ri += stride;
        oi += stride;
    }

    // PHASE 4: FINAL WRITES WITH SMALL WINDOW
    for _ in 0..phase_4_nreps {
        sum -= invec[li];
        current_window_size -= 1;
        outvec[oi] = sum / current_window_size as f32;
        li += stride;
        oi += stride;
    }
}

/// Upstream `boxAlongRowsFloat`.
fn box_along_rows_float(
    input: &[f32],
    out: &mut [f32],
    num_rows: usize,
    num_cols: usize,
    window_size: usize,
) {
    for i in 0..num_rows {
        box1d_float(
            &input[i * num_cols..],
            &mut out[i * num_cols..],
            num_cols,
            1,
            window_size,
        );
    }
}

/// Upstream `boxAlongColsFloat`.
fn box_along_cols_float(
    input: &[f32],
    out: &mut [f32],
    num_rows: usize,
    num_cols: usize,
    window_size: usize,
) {
    for j in 0..num_cols {
        box1d_float(&input[j..], &mut out[j..], num_rows, num_cols, window_size);
    }
}

/// Upstream `decimateFloat`: picks the pixel at the centre of each output cell.
pub(crate) fn decimate_float(
    input: &[f32],
    in_num_rows: usize,
    in_num_cols: usize,
    out: &mut [f32],
    out_num_rows: usize,
    out_num_cols: usize,
) {
    for outi in 0..out_num_rows {
        let ini = (((outi as f64 + 0.5) * in_num_rows as f64) / out_num_rows as f64) as usize;
        for outj in 0..out_num_cols {
            let inj = (((outj as f64 + 0.5) * in_num_cols as f64) / out_num_cols as f64) as usize;
            out[outi * out_num_cols + outj] = input[ini * in_num_cols + inj];
        }
    }
}

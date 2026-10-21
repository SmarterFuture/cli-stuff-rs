use std::{hint::unreachable_unchecked, mem::transmute};

#[inline]
const fn cmp_ge<const N: usize>(x: u64) -> u64 {
    const { assert!(N.is_power_of_two() && N <= 64) }

    let lane_max = u64::MAX >> (64 - N);
    let lsb = u64::MAX / lane_max;
    let h = lsb << (N - 1);
    let k = (N.ilog2() << 1) as usize;

    match N {
        2 | 4 => x & h,
        _ => ((x << (N - k)) | (x << (N - k - 1))) & h,
    }
}

#[inline]
const fn lane_popcount<const N: usize>(x: u64) -> u64 {
    const { assert!(N.is_power_of_two()) }

    const M3: u64 = 0x3333_3333_3333_3333;
    const M5: u64 = 0x5555_5555_5555_5555;
    const MF: u64 = 0x0F0F_0F0F_0F0F_0F0F;
    const MFF: u64 = 0x00FF_00FF_00FF_00FF;

    match N {
        2 => ((x >> 1) & M5) + (x & M5),
        4 => {
            let v = lane_popcount::<2>(x);
            (v & M3) + ((v >> 2) & M3)
        }
        8 => {
            let v = lane_popcount::<4>(x);
            (v & MF) + ((v >> 4) & MF)
        }
        16 => {
            let v = lane_popcount::<8>(x);
            (v & MFF) + ((v >> 8) & MFF)
        }
        32 => unsafe {
            let v = transmute::<u64, [u32; 2]>(x);
            transmute::<[u32; 2], u64>([v[0].count_ones(), v[1].count_ones()])
        },
        64 => x.count_ones() as u64,
        _ => unsafe { unreachable_unchecked() },
    }
}

#[inline]
const fn saturating_add<const N: usize>(a: u64, b: u64) -> u64 {
    const { debug_assert!(N == 2 || N == 4) }

    let h: u64 = match N {
        2 => 0xAAAA_AAAA_AAAA_AAAA,
        _ => 0x8888_8888_8888_8888,
    };

    let low = (a & !h).wrapping_add(b & !h);
    let ov = ((a & b) | ((a | b) & low)) & h;
    let sum = low ^ ((a ^ b) & h);
    let fill = ov | (ov - (ov >> (N - 1)));

    sum | fill
}

#[inline]
fn combine<const N: usize>(acc: &mut u64, val: u64) {
    const { assert!(N.is_power_of_two() && N > 1) }
    let pop = lane_popcount::<N>(val);

    match N {
        2 | 4 => *acc = saturating_add::<N>(*acc, pop),
        _ => *acc = acc.wrapping_add(pop),
    };
}

/// returns (data, junk)
/// length of data is always divisible by 4w
pub fn downsize<const N: usize>(w: usize, src: &mut [u64]) -> (&mut [u64], &mut [u64]) {
    const { assert!(N.is_power_of_two() && N > 1) }
    assert!(w > 0 && src.len().is_multiple_of(w));

    let mut base = 0;
    // we start at 1 bc the first line is in-place mapped
    let mut phase = 1;

    // only in-place mapping
    for o in &mut src[..w] {
        *o = lane_popcount::<N>(*o);
    }

    // well behaved full rows
    for y in (w..src.len()).step_by(w) {
        let (head, tail) = src.split_at_mut(y);
        let out = &mut head[base..base + w]; // base < y always here
        let row = &tail[..w];

        if phase == 0 {
            for x in 0..w {
                out[x] = lane_popcount::<N>(row[x]);
            }
        } else if phase == N - 1 {
            for x in 0..w {
                let mut a = out[x];
                combine::<N>(&mut a, row[x]);
                out[x] = cmp_ge::<N>(a);
            }
        } else {
            for x in 0..w {
                combine::<N>(&mut out[x], row[x]);
            }
        }

        phase += 1;
        if phase == N {
            phase = 0;
            base += w;
        }
    }

    if phase != 0 {
        for o in &mut src[base..base + w] {
            *o = cmp_ge::<N>(*o);
        }
        base += w;
    }

    src.split_at_mut(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturating_add_swar4() {
        assert_eq!(saturating_add::<4>(0x7777, 0x1111), 0x8888);
        assert_eq!(saturating_add::<4>(0xFFFF, 0x0000), 0xFFFF);
        assert_eq!(saturating_add::<4>(0xFFFF, 0x1111), 0xFFFF);
        assert_eq!(saturating_add::<4>(0x9999, 0x3333), 0xCCCC);
        assert_eq!(saturating_add::<4>(0xBBBB, 0xCCCC), 0xFFFF);
        assert_eq!(saturating_add::<4>(0x9F17, 0x3128), 0xCF3F);
    }

    #[test]
    fn saturating_add_swar2() {
        assert_eq!(saturating_add::<2>(0x5555, 0x5555), 0xAAAA);
        assert_eq!(saturating_add::<2>(0xFFFF, 0x0000), 0xFFFF);
        assert_eq!(saturating_add::<2>(0xFFFF, 0x5555), 0xFFFF);
        assert_eq!(saturating_add::<2>(0xAAAA, 0x5555), 0xFFFF);
        assert_eq!(saturating_add::<2>(0xAAAA, 0xAAAA), 0xFFFF);
        assert_eq!(saturating_add::<2>(0x6C, 0x93), 0xFF);
    }

    // ---- N = 2 (thr = 2; an "on" lane is 0b10) ----

    #[test]
    fn n2_single_group() {
        // lane0: 0b11 + 0b00 = 2 -> on; lane1: 0b01 + 0b00 = 1 -> off
        let mut s = [0b0111, 0b0000];
        downsize::<2>(1, &mut s);
        assert_eq!(s[..1], [0b0010]);
    }

    #[test]
    fn n2_exact_threshold_split_across_rows() {
        // 1 + 1 = 2 -> on
        let mut s = [0b01, 0b01];
        downsize::<2>(1, &mut s);
        assert_eq!(s[..1], [0b10]);
    }

    #[test]
    fn n2_saturation_still_on() {
        // 2 + 2 = 4 saturates to 3, still >= 2
        let mut s = [0b11, 0b11];
        downsize::<2>(1, &mut s);
        assert_eq!(s[..1], [0b10]);
    }

    #[test]
    fn n2_two_groups_width_two_compacts() {
        // rows: A0 A1 / B0 B1 / C0 C1 / D0 D1
        let mut s = [
            0b11, 0b01, // A
            0b00, 0b01, // B
            0b01, 0b11, // C
            0b00, 0b11, // D
        ];
        let out = downsize::<2>(2, &mut s).0;
        // group0: x0 = 2+0 on, x1 = 1+1 on; group1: x0 = 1+0 off, x1 = 2+2 on
        assert_eq!(out, [0b10, 0b10, 0, 0b10]);
    }

    // ---- N = 4 (thr = 8; an "on" lane is 0x8) ----

    #[test]
    fn n4_exactly_threshold_on() {
        let mut s = [0xF, 0xF, 0, 0]; // 4 + 4 = 8
        downsize::<4>(1, &mut s);
        assert_eq!(s[..1], [0x8]);
    }

    #[test]
    fn n4_one_below_threshold_off() {
        let mut s = [0xF, 0x7, 0, 0]; // 4 + 3 = 7
        downsize::<4>(1, &mut s);
        assert_eq!(s[..1], [0x0]);
    }

    #[test]
    fn n4_full_block_saturates_but_on() {
        let mut s = [0xF; 4]; // 16 saturates to 15
        downsize::<4>(1, &mut s);
        assert_eq!(s[..1], [0x8]);
    }

    // ---- N = 8 (thr = 32; an "on" lane is 0x80) ----

    #[test]
    fn n8_threshold_on() {
        let mut s = [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]; // 32
        downsize::<8>(1, &mut s);
        assert_eq!(s[..1], [0x80]);
    }

    #[test]
    fn n8_below_threshold_off() {
        let mut s = [0xFF, 0xFF, 0xFF, 0x7F, 0, 0, 0, 0]; // 31
        downsize::<8>(1, &mut s);
        assert_eq!(s[..1], [0]);
    }

    #[test]
    fn n8_lanes_independent() {
        // lane0 gets 32 (on), lane1 gets 31 (off)
        let mut s = [0xFFFF, 0xFFFF, 0xFFFF, 0x7FFF, 0, 0, 0, 0];
        downsize::<8>(1, &mut s);
        assert_eq!(s[..1], [0x0080]);
    }

    #[test]
    fn n8_all_ones_all_zeros() {
        let mut ones = [u64::MAX; 8];
        downsize::<8>(1, &mut ones);
        assert_eq!(ones[..1], [0x8080_8080_8080_8080]);

        let mut zeros = [0u64; 8];
        downsize::<8>(1, &mut zeros);
        assert_eq!(zeros[..1], [0]);
    }

    // ---- N = 64 (one lane, thr = 2048; "on" is the top bit) ----

    #[test]
    fn n64_half_on_and_one_less() {
        let mut s = [0u64; 64];
        s[..32].fill(u64::MAX); // 32 * 64 = 2048
        downsize::<64>(1, &mut s);
        assert_eq!(s[0], 1 << 63);

        let mut t = [0u64; 64];
        t[..32].fill(u64::MAX);
        t[31] = u64::MAX >> 1; // 2047
        downsize::<64>(1, &mut t);
        assert_eq!(t[0], 0);
    }

    // ---- Ragged: not enough lines (missing rows count as zero) ----

    #[test]
    fn ragged_n2_partial_on() {
        // group0 on; partial row has 2 bits in lane0 -> 2 >= 2 -> on
        let mut s = [0b11, 0b00, 0b11];
        let out = downsize::<2>(1, &mut s).0;
        assert_eq!(out, [0b10, 0b10]);
    }

    #[test]
    fn ragged_n2_partial_off() {
        let mut s = [0b11, 0b11, 0b01]; // partial count 1 < 2
        let out = downsize::<2>(1, &mut s).0;
        assert_eq!(out, [0b10, 0]);
    }

    #[test]
    fn ragged_n2_width_two() {
        let mut s = [
            0b11, 0b00, 0b00, 0b00, 0b11, 0b01, // partial: x0 = 2 on, x1 = 1 off
        ];
        let out = downsize::<2>(2, &mut s).0;
        assert_eq!(out, [0b10, 0, 0b10, 0]);
    }

    #[test]
    fn ragged_n8_one_extra_row_off() {
        let mut s = [0xFF; 9]; // group0 = 64 on; partial = 8 < 32 off
        let out = downsize::<8>(1, &mut s).0;
        assert_eq!(out, [0x80, 0]);
    }

    #[test]
    fn ragged_n8_four_extra_rows_on() {
        let mut s = [0u64; 12];
        s[8..].fill(0xFF); // partial = 4 * 8 = 32 -> on
        let out = downsize::<8>(1, &mut s).0;
        assert_eq!(out, [0, 0x80]);
    }

    // ---- Returned slice length ----

    #[test]
    fn returned_len_is_ceil_rows_over_n_times_w() {
        let mut s = [0u64; 10 * 3]; // 10 rows, w = 3
        assert_eq!(downsize::<4>(3, &mut s).0.len(), 3 * 3); // ceil(10/4) = 3 rows
    }

    // ---- Bad shapes ----

    #[test]
    #[should_panic]
    fn incomplete_line_panics() {
        let mut s = [0u64; 5]; // w = 2 -> not a multiple of w
        downsize::<2>(2, &mut s);
    }

    #[test]
    #[should_panic]
    fn zero_width_panics() {
        let mut s = [0u64; 4];
        downsize::<2>(0, &mut s);
    }
}

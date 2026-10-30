use std::{
    hint::{assert_unchecked, unreachable_unchecked},
    mem::transmute,
};

use crate::bit_chunk::unchecked;

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

/// splits to two, [a..b] [b..]
#[inline]
unsafe fn split_offset(src: &mut [u64], a: usize, b: usize) -> (&mut [u64], &mut [u64]) {
    debug_assert!(a < b && b < src.len());

    let (s1, s2) = unsafe { src.split_at_mut_unchecked(b) };
    let (_, s1) = unsafe { s1.split_at_mut_unchecked(a) };
    (s1, s2)
}

#[inline]
fn downsize_band<const N: usize>(w: usize, src: &[u64], dst: &mut [u64], n: usize) {
    for i in 0..w {
        let mut a = lane_popcount::<N>(unchecked!(src[i]));
        for k in 1..n {
            combine::<N>(&mut a, unchecked!(src[k * w + i]));
        }
        unchecked!(dst[i] = cmp_ge::<N>(a));
    }
}

#[inline]
fn downsize_band_overlap<const N: usize>(w: usize, src: &mut [u64], band: usize, n: usize) {
    let src_off = band * N * w;

    for i in 0..w {
        let mut a = lane_popcount::<N>(unchecked!(src[src_off + i]));
        for k in 1..n {
            combine::<N>(&mut a, unchecked!(src[src_off + k * w + i]));
        }
        unchecked!(src[src_off + i] = cmp_ge::<N>(a));
    }
}

/// returns (data, free)
/// length of data is always divisible by 4w
/// free buffer is non zero, and should not be assumed it is cleared
pub fn downsize<const N: usize>(w: usize, src: &mut [u64]) -> (&mut [u64], &mut [u64]) {
    const { assert!(N.is_power_of_two() && N > 1) }
    let total = src.len();

    assert!(w > 0 && total.is_multiple_of(w));

    let rows = total / w;
    let bands = rows / N;
    let rem = rows % N;
    let write = (bands + (rem > 0) as usize) * w;
    let padded = write.next_multiple_of(4 * w);

    assert!(padded <= total);

    if bands > 0 {
        downsize_band_overlap::<N>(w, src, 0, N);
    } else if rem > 0 {
        downsize_band_overlap::<N>(w, src, 0, rem);
    }

    for band in 1..bands {
        let (dst, data) = unsafe { split_offset(src, band * w, band * w * N) };
        downsize_band::<N>(w, data, dst, N);
    }

    if bands > 0 && rem > 0 {
        let (dst, data) = unsafe { split_offset(src, bands * w, bands * w * N) };
        downsize_band::<N>(w, data, dst, rem);
    }

    src[write..padded].fill(0);
    src.split_at_mut(padded)
}

pub fn downsize2(w: usize, src: &mut [u64]) -> (&mut [u64], &mut [u64]) {
    downsize::<2>(w, src)
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
        // 2 rows, w=1, N=2 -> 1 output row, padded to 4
        let mut s = [0b0111, 0b0000, 0, 0];
        let out = downsize::<2>(1, &mut s).0;
        // lane0: popcount(0b11) + popcount(0b00) = 2 -> on
        // lane1: popcount(0b01) + popcount(0b00) = 1 -> off
        assert_eq!(out, [0b10, 0, 0, 0]);
    }

    #[test]
    fn n2_exact_threshold_split_across_rows() {
        // 1 + 1 = 2 -> on
        let mut s = [0b01, 0b01, 0, 0];
        let out = downsize::<2>(1, &mut s).0;
        assert_eq!(out, [0b10, 0, 0, 0]);
    }

    #[test]
    fn n2_saturation_still_on() {
        // 2 + 2 = 4 saturates to 3, still >= 2
        let mut s = [0b11, 0b11, 0, 0];
        let out = downsize::<2>(1, &mut s).0;
        assert_eq!(out, [0b10, 0, 0, 0]);
    }

    #[test]
    fn n2_two_groups_width_two_compacts() {
        // 4 rows, w=2, N=2 -> 2 output rows, padded to 8
        let mut s = [
            0b11, 0b01, // A
            0b00, 0b01, // B
            0b01, 0b11, // C
            0b00, 0b11, // D
        ];
        let out = downsize::<2>(2, &mut s).0;
        // group0: x0 = 2+0 on, x1 = 1+1 on; group1: x0 = 1+0 off, x1 = 2+2 on
        assert_eq!(out, [0b10, 0b10, 0, 0b10, 0, 0, 0, 0]);
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
        // 3 rows, w=1, N=2 -> 1 full group + 1 partial, padded to 4
        let mut s = [0b11, 0b00, 0b11, 0];
        let out = downsize::<2>(1, &mut s).0;
        assert_eq!(out, [0b10, 0b10, 0, 0]);
    }

    #[test]
    fn ragged_n2_partial_off() {
        // partial count 1 < 2 -> off
        let mut s = [0b11, 0b11, 0b01, 0];
        let out = downsize::<2>(1, &mut s).0;
        assert_eq!(out, [0b10, 0, 0, 0]);
    }

    #[test]
    fn ragged_n2_width_two() {
        // 3 rows, w=2, N=2 -> 1 full group + 1 partial, padded to 8
        let mut s = [
            0b11, 0b00, 0b00, 0b00, 0b11, 0b01, // rows 0-2
            0, 0, // padding
        ];
        let out = downsize::<2>(2, &mut s).0;
        // partial: x0 = 2 on, x1 = 1 off
        assert_eq!(out, [0b10, 0, 0b10, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn ragged_n8_one_extra_row_off() {
        // 9 rows, w=1, N=8 -> 1 full group + 1 partial (count 8 < 32 off), padded to 4
        let mut s = [0xFF; 9];
        let out = downsize::<8>(1, &mut s).0;
        assert_eq!(out, [0x80, 0, 0, 0]);
    }

    #[test]
    fn ragged_n8_four_extra_rows_on() {
        // 12 rows, w=1, N=8 -> group0 zero, partial = 4*8 = 32 -> on, padded to 4
        let mut s = [0u64; 12];
        s[8..].fill(0xFF);
        let out = downsize::<8>(1, &mut s).0;
        assert_eq!(out, [0, 0x80, 0, 0]);
    }

    // ---- Returned slice length ----

    #[test]
    fn returned_len_is_padded_to_4w() {
        // 10 rows, w=3, N=4 -> ceil(10/4)=3 output rows -> 9 cells -> padded to 12
        let mut s = [0u64; 10 * 3];
        assert_eq!(downsize::<4>(3, &mut s).0.len(), 12);
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

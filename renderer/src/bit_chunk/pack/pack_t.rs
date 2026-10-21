use std::{
    hint::unreachable_unchecked,
    slice::{from_raw_parts, from_raw_parts_mut},
};

use crate::bit_chunk::pack::unchecked;

/// compress for lanes of size 8, 16, 32 (byte, word, long)
#[inline]
fn compress_bwl<const N: usize>(a: u64) -> u64 {
    const { assert!(N.is_power_of_two()) }
    const { assert!(N >= 8 && N <= 32) }

    match N {
        8 => {
            a & (0xFF << 56)
                | (a << 8) & (0xFF << 48)
                | (a << 16) & (0xFF << 40)
                | (a << 24) & (0xFF << 32)
        }
        16 => a & (0xFF << 56) | (a << 24) & (0xFF << 48),
        32 => a & (0xFF << 56),
        _ => unsafe { unreachable_unchecked() },
    }
}

/// pack operation for lanes of size 8, 16, 32 (byte, word, long)
pub(super) fn pack_bwl<'a, const N: usize>(
    w: usize,
    src: &mut [u64],
    dst: &'a mut [u64],
) -> &'a [u8] {
    const { assert!(N.is_power_of_two()) }
    const { assert!(N >= 8 && N <= 32) }
    assert!(w <= usize::MAX / 64);

    let step: u8 = (128 / N as u8) << 1;
    let total = src.len();

    assert!(w > 0 && total.is_multiple_of(w));

    let bands = (total / w).div_ceil(4);
    let size = (bands * w * 4).div_ceil(N);

    assert!(dst.len() >= size);

    let mut base = 0;
    let mut phase: u8 = 0;
    let mut next = 0;
    let mut shift: u8 = 0;

    let mut word = 0;

    for y in (0..total).step_by(w) {
        for x in 0..w {
            let acc = src[base + x] | (src[y + x] >> phase);
            src[base + x] = acc;

            if phase == 6 || y + w == total {
                let done = acc | (acc << (N - 1));
                word |= compress_bwl::<N>(done) >> shift;
                shift += step;
                if shift == 64 {
                    dst[next] = word.to_be();
                    next += 1;
                    word = 0;
                    shift = 0;
                }
            }
        }

        phase += 2;
        if phase == 8 {
            phase = 0;
            base += w * 4;
        }
    }

    let mut len = next * 8;
    if shift != 0 {
        dst[next] = word.to_be();
        len += (shift / 8) as usize;
    }

    unsafe { from_raw_parts(dst.as_ptr() as *const u8, len) }
}

/// pack operation for lanes of size 64 (quad)
pub(super) fn pack_q<'a>(w: usize, src: &[u64], dst: &'a mut [u64]) -> &'a [u8] {
    assert!(w <= usize::MAX / 64);

    let total = src.len();

    assert!(w > 0 && total.is_multiple_of(w));

    let bw = w.div_ceil(2);
    let size = (total / w).div_ceil(4) * bw;
    assert!(dst.len() * 8 >= size);

    let mut base = 0;
    let mut phase = 0;

    let out = unsafe { from_raw_parts_mut(dst.as_mut_ptr() as *mut u8, size) };

    for y in (0..total).step_by(w) {
        for x in 0..w {
            let col = x & 1;
            let pixel = (unchecked!(src[y + x]) >> 56) as u8;
            let offset = base + (x >> 1);

            if phase == 0 && col == 0 {
                unchecked!(out[offset] = pixel);
                continue;
            }

            unchecked!(out[offset] |= pixel >> (phase + col));
        }

        phase += 2;
        if phase == 8 {
            phase = 0;
            base += bw;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- N = 8: a c e g → aceg0000 ----------
    // input bytes (MSB→LSB): a b c d e f g h
    // output bytes:          a c e g 0 0 0 0

    #[test]
    fn n8_identity_when_odd_fields_zero() {
        assert_eq!(
            compress_bwl::<8>(0x0100_0200_0300_0400),
            0x0102_0304_0000_0000
        );
    }

    #[test]
    fn n8_drops_odd_fields() {
        assert_eq!(
            compress_bwl::<8>(0xAA11_BB22_CC33_DD44),
            0xAABB_CCDD_0000_0000
        );
    }

    #[test]
    fn n8_odd_fields_only_yields_zero() {
        assert_eq!(compress_bwl::<8>(0x0011_0022_0033_0044), 0);
    }

    #[test]
    fn n8_all_ones() {
        assert_eq!(compress_bwl::<8>(u64::MAX), 0xFFFF_FFFF_0000_0000);
    }

    #[test]
    fn n8_zero() {
        assert_eq!(compress_bwl::<8>(0), 0);
    }

    #[test]
    fn n8_sequential() {
        // a=01 b=02 c=03 d=04 e=05 f=06 g=07 h=08
        assert_eq!(
            compress_bwl::<8>(0x0102_0304_0506_0708),
            0x0103_0507_0000_0000
        );
    }

    // ---------- N = 16: a c → aacc0000… ----------
    // input bytes: a ? b ? c ? d ?   (a@0, b@2, c@4, d@6)
    // output bytes: a c 0 0 0 0 0 0

    #[test]
    fn n16_drops_odd_lanes() {
        assert_eq!(
            compress_bwl::<16>(0xAAAA_BBBB_CCCC_DDDD),
            0xAACC_0000_0000_0000
        );
    }

    #[test]
    fn n16_only_a() {
        assert_eq!(
            compress_bwl::<16>(0xAAAA_BBBB_0000_0000),
            0xAA00_0000_0000_0000
        );
    }

    #[test]
    fn n16_only_c_moves_to_byte_1() {
        assert_eq!(
            compress_bwl::<16>(0x0000_0000_CCCC_DDDD),
            0x00CC_0000_0000_0000
        );
    }

    #[test]
    fn n16_odd_lanes_only_yields_zero() {
        assert_eq!(compress_bwl::<16>(0x0000_BBBB_0000_DDDD), 0);
    }

    #[test]
    fn n16_all_ones() {
        assert_eq!(compress_bwl::<16>(u64::MAX), 0xFFFF_0000_0000_0000);
    }

    #[test]
    fn n16_zero() {
        assert_eq!(compress_bwl::<16>(0), 0);
    }

    #[test]
    fn n16_each_even_lane_individually() {
        assert_eq!(
            compress_bwl::<16>(0xDEAD_0000_0000_0000),
            0xDE00_0000_0000_0000
        );
        assert_eq!(
            compress_bwl::<16>(0x0000_0000_BEEF_0000),
            0x00BE_0000_0000_0000
        );
    }

    // ---------- N = 32: a → a0000000 ----------
    // input bytes: a ? ? ? b ? ? ?
    // output bytes: a 0 0 0 0 0 0 0

    #[test]
    fn n32_keeps_top_byte() {
        assert_eq!(
            compress_bwl::<32>(0xAAAA_AAAA_BBBB_BBBB),
            0xAA00_0000_0000_0000
        );
    }

    #[test]
    fn n32_bottom_lane_only_yields_zero() {
        assert_eq!(compress_bwl::<32>(0x0000_0000_FFFF_FFFF), 0);
    }

    #[test]
    fn n32_all_ones() {
        assert_eq!(compress_bwl::<32>(u64::MAX), 0xFF00_0000_0000_0000);
    }

    #[test]
    fn n32_zero() {
        assert_eq!(compress_bwl::<32>(0), 0);
    }

    // ---------- Cross-cutting ----------

    #[test]
    fn output_never_exceeds_its_mask() {
        let x = 0xDEAD_BEEF_CAFE_BABEu64;
        assert_eq!(compress_bwl::<8>(x) & !0xFFFF_FFFF_0000_0000, 0);
        assert_eq!(compress_bwl::<16>(x) & !0xFFFF_0000_0000_0000, 0);
        assert_eq!(compress_bwl::<32>(x) & !0xFF00_0000_0000_0000, 0);
    }
}

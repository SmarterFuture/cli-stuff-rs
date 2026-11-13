#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    _mm_storeu_si128, _mm256_castsi256_si128, _mm256_loadu_si256, _mm256_or_si256,
    _mm256_permute4x64_epi64, _mm256_setr_epi8, _mm256_shuffle_epi8, _mm256_slli_epi16,
    _mm256_srli_epi64, _mm256_storeu_si256,
};
use std::{
    array::from_fn,
    slice::{from_raw_parts, from_raw_parts_mut},
};

#[cfg(target_arch = "x86_64")]
use crate::bit_chunk::pack::avx_helpers::load_block;
use crate::bit_chunk::pack::bwl_scalar::pack_bwl_band;

#[inline]
#[cfg(target_endian = "little")]
#[target_feature(enable = "avx2")]
unsafe fn pack_b_4by4_avx2(rows_ptr: &[*const u64; 4], x: usize, dst_ptr: *mut u8) {
    unsafe {
        #[rustfmt::skip]
        let mask = _mm256_setr_epi8(
            7, 5, 3, 1, 15, 13, 11, 9,
            -1, -1, -1, -1, -1, -1, -1, -1,
            7, 5, 3, 1, 15, 13, 11, 9,
            -1, -1, -1, -1, -1, -1, -1, -1,
        );

        let a = load_block!(rows_ptr, x, 2, 4, 6);
        let ta = _mm256_slli_epi16::<7>(a);
        let ta = _mm256_or_si256(ta, a);

        let u = _mm256_shuffle_epi8(ta, mask);

        let c = _mm256_permute4x64_epi64::<0x08>(u);

        let out = _mm256_castsi256_si128(c);

        _mm_storeu_si128(dst_ptr as _, out);
    }
}

#[inline]
#[cfg(target_endian = "little")]
#[target_feature(enable = "avx2")]
unsafe fn pack_b_8by4_avx2(rows_ptr: &[*const u64; 4], x: usize, dst_ptr: *mut u8) {
    unsafe {
        #[rustfmt::skip]
        let mask_a = _mm256_setr_epi8(
            7, 5, 3, 1, 15, 13, 11, 9,
            -1, -1, -1, -1, -1, -1, -1, -1,
            7, 5, 3, 1, 15, 13, 11, 9,
            -1, -1, -1, -1, -1, -1, -1, -1,
        );

        #[rustfmt::skip]
        let mask_b = _mm256_setr_epi8(
            -1, -1, -1, -1, -1, -1, -1, -1,
            7, 5, 3, 1, 15, 13, 11, 9,
            -1, -1, -1, -1, -1, -1, -1, -1,
            7, 5, 3, 1, 15, 13, 11, 9,
        );

        let a = load_block!(rows_ptr, x, 2, 4, 6);
        let b = load_block!(rows_ptr, x + 4, 2, 4, 6);

        let ta = _mm256_or_si256(_mm256_slli_epi16::<7>(a), a);
        let tb = _mm256_or_si256(_mm256_slli_epi16::<7>(b), b);

        let ca = _mm256_shuffle_epi8(ta, mask_a);
        let cb = _mm256_shuffle_epi8(tb, mask_b);

        let c = _mm256_or_si256(ca, cb);

        // in [a, c, b, d] -> [a->a, c->b, b->c, d->d] -> 1101_1000 (reverse order)
        let out = _mm256_permute4x64_epi64::<0xD8>(c);

        _mm256_storeu_si256(dst_ptr as _, out);
    }
}

/// pack operation for lanes of size 8 (byte)
pub(super) fn pack_b<'a>(w: usize, src: &[u64], dst: &'a mut [u64]) -> &'a [u8] {
    assert!(w <= usize::MAX / 4);
    let total = src.len();
    assert!(w > 0 && total.is_multiple_of(w * 4));

    let bands = total / (w * 4);
    let step = 4 * w;
    let size = bands * step;

    assert!(dst.len() * 8 >= size);

    let out = unsafe { from_raw_parts_mut(dst.as_mut_ptr() as *mut u8, size) };

    let src_ptr = src.as_ptr();

    let dst_ptr = out.as_mut_ptr();
    let mut base = 0;

    #[cfg(all(target_arch = "x86_64", target_endian = "little"))]
    let avx2 = is_x86_feature_detected!("avx2");

    for band in 0..bands {
        // FUTURE: maybe do not recompute but just add, maybe it will reuse old regs that already hold this
        let rows: [*const u64; 4] = unsafe { from_fn(|r| src_ptr.add(band * step + r * w)) };

        #[cfg(all(target_arch = "x86_64", target_endian = "little"))]
        if avx2 && w >= 4 {
            let main = w & !7;
            for x in (0..main).step_by(8) {
                unsafe {
                    pack_b_8by4_avx2(&rows, x, dst_ptr.add(base + x * 4));
                }
            }

            let rem = w & 7;
            match rem {
                0 => {}
                1..=4 => unsafe {
                    pack_b_4by4_avx2(&rows, w - 4, dst_ptr.add(base + step - 16));
                },
                _ if w < 8 => unsafe {
                    pack_b_4by4_avx2(&rows, 0, dst_ptr.add(base));
                    pack_b_4by4_avx2(&rows, w - 4, dst_ptr.add(base + step - 16));
                },
                _ => unsafe {
                    pack_b_8by4_avx2(&rows, w - 8, dst_ptr.add(base + step - 32));
                },
            }
        } else {
            unsafe { pack_bwl_band::<8, 4>(&rows, 0, dst_ptr.add(base), w) };
        }

        #[cfg(not(all(target_arch = "x86_64", target_endian = "little")))]
        unsafe {
            pack_bwl_band::<8, 4>(&rows, 0, dst_ptr.add(base), w)
        };

        base += step;
    }

    unsafe { from_raw_parts(dst.as_ptr() as *const u8, size) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "10110010" -> u64: pixel `p` (0 = leftmost) sits at bit `63 - p*8`.
    fn word(pixels: &str) -> u64 {
        assert!(pixels.len() <= 8);
        let mut v = 0u64;
        for (p, c) in pixels.bytes().enumerate() {
            if c == b'1' {
                v |= 1u64 << (63 - p * 8);
            }
        }
        v
    }

    fn pack_b_reference(w: usize, src: &[u64]) -> Vec<u8> {
        let bands = src.len() / (4 * w);
        let mut out = vec![0u8; bands * w * 4];
        for b in 0..bands {
            for i in 0..w {
                for k in 0..4 {
                    let mut byte = 0u8;
                    for (row, dx) in [(0usize, 0u8), (1, 1), (2, 2), (3, 3)] {
                        let word = src[(b * 4 + row) * w + i];
                        // pixel 2k   -> bits {7,5,3,1} of output byte k
                        if (word >> (63 - (2 * k) * 8)) & 1 == 1 {
                            byte |= 1 << (7 - 2 * dx);
                        }
                        // pixel 2k+1 -> bits {6,4,2,0} of output byte k
                        if (word >> (63 - (2 * k + 1) * 8)) & 1 == 1 {
                            byte |= 1 << (6 - 2 * dx);
                        }
                    }
                    out[(b * w + i) * 4 + k] = byte;
                }
            }
        }
        out
    }

    /// Run pack_b with a 1-word guard after `dst[..words]`; verify no overrun.
    fn run(w: usize, src: &[u64]) -> Vec<u8> {
        let bands = src.len() / (4 * w);
        let len = bands * w * 4;
        let words = len.div_ceil(8);
        let mut dst = vec![0xAAAA_AAAA_AAAA_AAAAu64; words + 1];
        let out = pack_b(w, src, &mut dst[..words]).to_vec();

        assert_eq!(out.len(), len, "returned length");

        let bytes: Vec<u8> = dst.iter().flat_map(|v| v.to_ne_bytes()).collect();
        assert!(
            bytes[len..].iter().all(|&b| b == 0xAA),
            "wrote past the end (len={len})",
        );
        out
    }

    fn test(rows: &[&str], expected: &[u8]) {
        assert_eq!(rows.len(), 4);
        let src: Vec<u64> = rows.iter().map(|r| word(r)).collect();
        assert_eq!(run(1, &src), expected);
    }

    // ---------- Hand-computed ----------

    #[test]
    fn pack_b_hand_computed() {
        test(&["10110010"; 4], &[0xAA, 0xFF, 0x00, 0xAA]);
    }

    // ---------- All set / all clear ----------

    #[test]
    fn pack_b_all_set_and_all_clear() {
        test(&["11111111"; 4], &[0xFF; 4]);
        test(&["00000000"; 4], &[0; 4]);
    }

    #[test]
    fn pack_b_left_pixel_is_the_high_bit() {
        test(&["10000000"; 4], &[0xAA, 0, 0, 0]);
        test(&["01000000"; 4], &[0x55, 0, 0, 0]);
    }

    // ---------- Single-pixel sweep ----------

    #[test]
    fn pack_b_single_pixel_sweep() {
        for w in 1..=5usize {
            for bands in 1..=2usize {
                for band in 0..bands {
                    for row in 0..4 {
                        for x in 0..w {
                            for p in 0..8 {
                                let mut src = vec![0u64; bands * 4 * w];
                                src[(band * 4 + row) * w + x] = 1u64 << (63 - p * 8);

                                assert_eq!(
                                    run(w, &src),
                                    pack_b_reference(w, &src),
                                    "w={w} bands={bands} band={band} row={row} x={x} p={p}",
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn pack_b_reference_agrees() {
        for w in [1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 13, 14, 15, 16] {
            for bands in [1, 2, 3] {
                let mut src = vec![0u64; bands * 4 * w];
                for (i, word) in src.iter_mut().enumerate() {
                    *word = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) & 0x8080_8080_8080_8080;
                }
                assert_eq!(
                    run(w, &src),
                    pack_b_reference(w, &src),
                    "w={w} bands={bands}"
                );
            }
        }
    }

    // ---------- Panic paths ----------

    #[test]
    #[should_panic]
    fn pack_b_total_not_multiple_of_4w_panics() {
        pack_b(1, &[0u64; 3], &mut [0u64; 4]);
    }

    #[test]
    #[should_panic]
    fn pack_b_w_zero_panics() {
        pack_b(0, &[0u64; 4], &mut [0u64; 4]);
    }

    #[test]
    #[should_panic]
    fn pack_b_dst_too_small_panics() {
        pack_b(1, &[0u64; 4], &mut []);
    }
}

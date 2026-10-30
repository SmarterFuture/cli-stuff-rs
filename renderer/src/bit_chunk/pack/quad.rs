use std::{array::from_fn, slice::from_raw_parts_mut};

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    _mm256_loadu_si256, _mm256_movemask_epi8, _mm256_or_si256, _mm256_permute4x64_epi64,
    _mm256_setr_epi8, _mm256_shuffle_epi8, _mm256_srli_epi64,
};

#[cfg(target_arch = "x86_64")]
use crate::bit_chunk::pack::avx_helpers::loadbur_block;
use crate::bit_chunk::unchecked;

/// 4 columns of 4 rows -> 2 bytes
#[inline]
#[cfg(target_endian = "little")]
#[target_feature(enable = "avx2")]
unsafe fn pack_q_4by4_avx2(rows_ptr: &[*const u64; 4], x: usize, dst_ptr: *mut u8) {
    unsafe {
        #[rustfmt::skip]
        let mask = _mm256_setr_epi8(
            15, 7, 13, 5, 11, 3, 9, 1,
            -1, -1, -1, -1, -1, -1, -1, -1,
            15, 7, 13, 5, 11, 3, 9, 1,
            -1, -1, -1, -1, -1, -1, -1, -1,
        );

        let t = loadbur_block!(rows_ptr, x, 6, 4, 2);

        let u = _mm256_shuffle_epi8(t, mask);
        let m = _mm256_movemask_epi8(u) as u32;
        let cell = (m & 0xFF) | ((m >> 8) & 0xFF00);
        (dst_ptr as *mut u16).write_unaligned(cell as u16);
    }
}

/// 8 columns of 4 rows -> 4 bytes
#[inline]
#[cfg(target_endian = "little")]
#[target_feature(enable = "avx2")]
unsafe fn pack_q_8by4_avx2(rows_ptr: &[*const u64; 4], x: usize, dst_ptr: *mut u8) {
    unsafe {
        use crate::bit_chunk::pack::avx_helpers::load_block;

        #[rustfmt::skip]
        let mask_a = _mm256_setr_epi8(
            15, 7, 13, 5, 11, 3, 9, 1,
            -1, -1, -1, -1, -1, -1, -1, -1,
            15, 7, 13, 5, 11, 3, 9, 1,
            -1, -1, -1, -1, -1, -1, -1, -1,
        );

        #[rustfmt::skip]
        let mask_b = _mm256_setr_epi8(
            -1, -1, -1, -1, -1, -1, -1, -1,
            9, 1, 11, 3, 13, 5, 15, 7,
            -1, -1, -1, -1, -1, -1, -1, -1,
            9, 1, 11, 3, 13, 5, 15, 7,
        );

        let ta = loadbur_block!(rows_ptr, x, 6, 4, 2);
        let tb = load_block!(rows_ptr, x + 4, 16, 32, 48);

        let c = _mm256_or_si256(
            _mm256_shuffle_epi8(ta, mask_a),
            _mm256_shuffle_epi8(tb, mask_b),
        );

        // in [a, c, b, d] -> [a->a, c->b, b->c, d->d] -> 1101_1000 (reverse order)
        let c = _mm256_permute4x64_epi64::<0xD8>(c);

        (dst_ptr as *mut u32).write_unaligned(_mm256_movemask_epi8(c) as u32);
    }
}

#[inline(always)]
unsafe fn pack_q_cols_scalar(rows: &[*const u64; 4], mut x: usize, w: usize, dst: *mut u8) {
    let mut p = x / 2;
    while x < w {
        let mut acc = 0u8;
        for (r, row_ptr) in rows.iter().enumerate() {
            let a = (unchecked!(*row_ptr[x]) >> 56) as u8;
            let b = if x + 1 < w {
                (unchecked!(*row_ptr[x + 1]) >> 56) as u8
            } else {
                0
            };
            acc |= (a | b >> 1) >> (2 * r);
        }
        unsafe { *dst.add(p) = acc };
        p += 1;
        x += 2;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn pack_q_band_avx2(src: *const u64, dst: *mut u8, w: usize, y: usize, base: usize) {
    let rows: [*const u64; 4] = unsafe { from_fn(|r| src.add(y + r * w)) };

    let out = unsafe { dst.add(base) };

    let main = w & !7;
    for x in (0..main).step_by(8) {
        unsafe { pack_q_8by4_avx2(&rows, x, out.add(x / 2)) };
    }

    let mut x = main;
    if w - x >= 4 {
        unsafe { pack_q_4by4_avx2(&rows, x, out.add(x / 2)) };
        x += 4;
    }

    unsafe { pack_q_cols_scalar(&rows, x, w, out) };
}

unsafe fn pack_q_band_scalar(src: *const u64, dst: *mut u8, w: usize, y: usize, base: usize) {
    let rows: [*const u64; 4] = unsafe { from_fn(|r| src.add(y + r * w)) };
    unsafe { pack_q_cols_scalar(&rows, 0, w, dst.add(base)) };
}

/// pack operation for lanes of size 64 (quad)
pub(super) fn pack_q<'a>(w: usize, src: &[u64], dst: &'a mut [u64]) -> &'a [u8] {
    assert!(w <= usize::MAX / 64);
    let total = src.len();
    assert!(w > 0 && total.is_multiple_of(w * 4));

    let bw = w.div_ceil(2);
    let size = (total / w).div_ceil(4) * bw;
    assert!(dst.len() * 8 >= size);

    let bands = total / (w * 4);

    let mut base = 0;

    let out = unsafe { from_raw_parts_mut(dst.as_mut_ptr() as *mut u8, size) };

    let src_ptr = src.as_ptr();
    let dst_ptr = out.as_mut_ptr();

    #[cfg(target_arch = "x86_64")]
    let avx2 = is_x86_feature_detected!("avx2");

    for band in 0..bands {
        let y = band * 4 * w;

        #[cfg(target_arch = "x86_64")]
        if avx2 {
            unsafe { pack_q_band_avx2(src_ptr, dst_ptr, w, y, base) };
            base += bw;
            continue;
        }

        unsafe { pack_q_band_scalar(src_ptr, dst_ptr, w, y, base) };
        base += bw;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_src(w: usize, pattern: &[bool]) -> Vec<u64> {
        assert_eq!(pattern.len(), 4 * w, "pattern must be 4 rows of w columns");
        pattern
            .iter()
            .map(|&b| if b { 1 << 63 } else { 0 })
            .collect()
    }

    #[test]
    fn all_zeros() {
        for w in 1..=20 {
            let src = make_src(w, &vec![false; 4 * w]);
            let mut dst = vec![0u64; w.div_ceil(16)];
            let out = pack_q(w, &src, &mut dst);
            assert_eq!(out, &vec![0u8; w.div_ceil(2)], "failed at w={w}");
        }
    }

    #[test]
    fn all_ones() {
        for w in 1..=20 {
            let src = make_src(w, &vec![true; 4 * w]);
            let mut dst = vec![0u64; w.div_ceil(16)];
            let out = pack_q(w, &src, &mut dst);

            let mut expected = vec![0xFF; w / 2];
            if w % 2 == 1 {
                expected.push(0xAA); // Odd column at the end only has left bit
            }
            assert_eq!(out, &expected, "failed at w={w}");
        }
    }

    #[test]
    fn single_bits() {
        // Test every single pixel position in a 4x16 grid
        for r in 0..4 {
            for c in 0..16 {
                let w = 16;
                let mut pattern = vec![false; 4 * w];
                pattern[r * w + c] = true;
                let src = make_src(w, &pattern);
                let mut dst = vec![0u64; 1];
                let out = pack_q(w, &src, &mut dst);

                let mut expected = vec![0u8; 8];
                let byte_idx = c / 2;
                let bit_idx = if c % 2 == 0 {
                    7 - 2 * r // Even columns use left bits (7, 5, 3, 1)
                } else {
                    6 - 2 * r // Odd columns use right bits (6, 4, 2, 0)
                };
                expected[byte_idx] = 1 << bit_idx;

                assert_eq!(out, &expected, "failed at row {r}, col {c}");
            }
        }
    }

    #[test]
    fn checkerboard() {
        // Alternating (r + c) % 2
        let w = 8;
        let mut pattern = vec![false; 4 * w];
        for r in 0..4 {
            for c in 0..w {
                if (r + c) % 2 == 0 {
                    pattern[r * w + c] = true;
                }
            }
        }
        let src = make_src(w, &pattern);
        let mut dst = vec![0u64; 1];
        let out = pack_q(w, &src, &mut dst);
        assert_eq!(out, &[0x99, 0x99, 0x99, 0x99]);
    }

    #[test]
    fn alternating_rows() {
        let w = 8;
        let mut pattern = vec![false; 4 * w];
        for r in 0..4 {
            if r % 2 == 0 {
                for c in 0..w {
                    pattern[r * w + c] = true;
                }
            }
        }
        let src = make_src(w, &pattern);
        let mut dst = vec![0u64; 1];
        let out = pack_q(w, &src, &mut dst);
        assert_eq!(out, &[0xCC, 0xCC, 0xCC, 0xCC]);
    }

    #[test]
    fn alternating_cols() {
        let w = 8;
        let mut pattern = vec![false; 4 * w];
        for r in 0..4 {
            for c in 0..w {
                if c % 2 == 0 {
                    pattern[r * w + c] = true;
                }
            }
        }
        let src = make_src(w, &pattern);
        let mut dst = vec![0u64; 1];
        let out = pack_q(w, &src, &mut dst);
        assert_eq!(out, &[0xAA, 0xAA, 0xAA, 0xAA]);
    }

    #[test]
    fn boundary_widths() {
        let widths = [1, 2, 3, 4, 5, 7, 8, 9, 11, 12, 13, 15, 16, 17];
        for &w in &widths {
            let src = make_src(w, &vec![true; 4 * w]);
            let mut dst = vec![0u64; w.div_ceil(16)];
            let out = pack_q(w, &src, &mut dst);

            let mut expected = vec![0xFF; w / 2];
            if w % 2 == 1 {
                expected.push(0xAA);
            }
            assert_eq!(out, &expected, "failed at boundary width w={w}");
        }
    }

    // AVX

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn isolated_columns_avx2() {
        if !is_x86_feature_detected!("avx2") {
            return; // Skip if CPU doesn't support it
        }

        let w = 16;
        let mut pattern = vec![false; 4 * w];

        // Light up Column 0 (byte 0), Column 8 (byte 4), and Column 15 (byte 7, right bit)
        for r in 0..4 {
            pattern[r * w] = true;
            pattern[r * w + 8] = true;
            pattern[r * w + 15] = true;
        }

        let src = make_src(w, &pattern);
        let mut dst = vec![0u64; 1];
        let out = pack_q(w, &src, &mut dst);

        let mut expected = vec![0u8; 8];
        expected[0] = 0xAA; // Col 0
        expected[4] = 0xAA; // Col 8
        expected[7] = 0x55; // Col 15 (odd column, bits 6,4,2,0)

        assert_eq!(out, &expected);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn avx2_exact_remainders() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }

        // Force the 8x4, 4x4, and scalar paths to run sequentially
        // w=13 -> 8x4 (x=0), 4x4 (x=8), scalar (x=12)
        let w = 13;
        let src = make_src(w, &vec![true; 4 * w]);
        let mut dst = vec![0u64; 1];
        let out = pack_q(w, &src, &mut dst);

        let mut expected = vec![0xFF; 6];
        expected.push(0xAA); // 13th column is odd (left bits only)
        assert_eq!(out, &expected);
    }
}

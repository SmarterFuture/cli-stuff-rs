use std::{array::from_fn, hint::unreachable_unchecked, slice::from_raw_parts};

use crate::bit_chunk::pack::unchecked;

#[inline(always)]
unsafe fn pack_n_band<const R: usize>(
    src_ptr: *const u64,
    dst_ptr: *mut u64,
    w: usize,
    y: usize,
    base: usize,
) {
    let row_ptrs: [*const u64; R] = unsafe { from_fn(|r| src_ptr.add(y + r * w)) };

    for x in 0..w {
        let mut acc = 0;

        for (r, &row_p) in row_ptrs.iter().enumerate() {
            let raw = unchecked!(*row_p[x]).to_be();
            let val = (raw | raw << 3) & 0xC0C0_C0C0_C0C0_C0C0;

            acc |= val >> (r << 1);
        }

        unchecked!(*dst_ptr[base + x] = acc);
    }
}

#[cold]
#[inline(never)]
unsafe fn pack_n_band_cold<const R: usize>(
    src_ptr: *const u64,
    dst_ptr: *mut u64,
    w: usize,
    y: usize,
    base: usize,
) {
    unsafe { pack_n_band::<R>(src_ptr, dst_ptr, w, y, base) };
}

/// pack operation for lanes of size 4 (nibble)
pub(super) fn pack_n<'a>(w: usize, src: &[u64], dst: &'a mut [u64]) -> &'a [u8] {
    assert!(w <= usize::MAX / 64);

    let total = src.len();

    assert!(w > 0 && total.is_multiple_of(w));

    let rows = total / w;
    let full_bands = rows / 4;
    let rem = rows % 4;
    let bands = full_bands + usize::from(rem > 0);
    assert!(dst.len() >= bands * w);

    let src_ptr = src.as_ptr();
    let dst_ptr = dst.as_mut_ptr();
    let mut base = 0;

    for band in 0..full_bands {
        unsafe { pack_n_band::<4>(src_ptr, dst_ptr, w, band * w * 4, base) };
        base += w;
    }

    match rem {
        0 => {}
        1 => unsafe { pack_n_band_cold::<1>(src_ptr, dst_ptr, w, full_bands * w * 4, base) },
        2 => unsafe { pack_n_band_cold::<2>(src_ptr, dst_ptr, w, full_bands * w * 4, base) },
        3 => unsafe { pack_n_band_cold::<3>(src_ptr, dst_ptr, w, full_bands * w * 4, base) },
        _ => unsafe { unreachable_unchecked() },
    }

    let len = (bands * w) * 8;
    unsafe { from_raw_parts(dst.as_ptr() as *const u8, len) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: u64 = 0x0101_0101_0101_0101; // multiply a byte value by this to splat it

    #[test]
    fn pack_n_full_band_zeros() {
        let src = [0u64; 4];
        let mut dst = [0u64; 1];
        let out = pack_n(1, &src, &mut dst);
        assert_eq!(out, &[0u8; 8]);
        assert_eq!(dst, [0]);
    }

    #[test]
    fn pack_n_full_band_ones() {
        // val = 0xC0 per byte; rows land at >>0, >>2, >>4, >>6 -> 0xC0|0x30|0x0C|0x03
        let src = [u64::MAX; 4];
        let mut dst = [0u64; 1];
        pack_n(1, &src, &mut dst);
        assert_eq!(dst, [0xFF * B]);
    }

    #[test]
    fn pack_n_three_rows() {
        let src = [u64::MAX; 3];
        let mut dst = [0u64; 1];
        pack_n(1, &src, &mut dst);
        assert_eq!(dst, [0xFC * B]);
    }

    #[test]
    fn pack_n_two_rows() {
        let src = [u64::MAX; 2];
        let mut dst = [0u64; 1];
        pack_n(1, &src, &mut dst);
        assert_eq!(dst, [0xF0 * B]);
    }

    #[test]
    fn pack_n_one_row() {
        let src = [u64::MAX; 1];
        let mut dst = [0u64; 1];
        pack_n(1, &src, &mut dst);
        assert_eq!(dst, [0xC0 * B]);
    }

    #[test]
    fn pack_n_row_position() {
        // only row k set -> its pair lands at 0xC0 >> 2k; all-ones data can't catch a bad shift
        for k in 0..4 {
            let mut src = [0u64; 4];
            src[k] = u64::MAX;
            let mut dst = [0u64; 1];
            pack_n(1, &src, &mut dst);
            assert_eq!(dst, [(0xC0u64 >> (2 * k)) * B], "row {k}");
        }
    }

    #[test]
    fn pack_n_two_full_bands() {
        let src = [u64::MAX; 8];
        let mut dst = [0u64; 2];
        let out = pack_n(1, &src, &mut dst);
        assert_eq!(out.len(), 16);
        assert_eq!(dst, [0xFF * B; 2]);
    }

    #[test]
    fn pack_n_band_plus_partial() {
        let src = [u64::MAX; 5];
        let mut dst = [0u64; 2];
        pack_n(1, &src, &mut dst);
        assert_eq!(dst, [0xFF * B, 0xC0 * B]);
    }

    #[test]
    fn pack_n_w2_two_rows() {
        // 4 cells at w=2 -> 2 rows, rem = 2
        let src = [u64::MAX; 4];
        let mut dst = [0u64; 2];
        let out = pack_n(2, &src, &mut dst);
        assert_eq!(out.len(), 16);
        assert_eq!(dst, [0xF0 * B; 2]);
    }

    #[test]
    fn pack_n_returns_correct_len() {
        for rows in 1..=8 {
            for w in 1..=4 {
                let src = vec![0u64; rows * w];
                let bands = rows.div_ceil(4);
                let mut dst = vec![0u64; bands * w];
                let out = pack_n(w, &src, &mut dst);
                assert_eq!(out.len(), bands * w * 8, "rows={rows}, w={w}");
            }
        }
    }
}

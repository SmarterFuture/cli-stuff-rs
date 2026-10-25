use std::{array::from_fn, slice::from_raw_parts};

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

/// pack operation for lanes of size 4 (nibble)
pub(super) fn pack_n<'a>(w: usize, src: &[u64], dst: &'a mut [u64]) -> &'a [u8] {
    assert!(w <= usize::MAX / 64);

    let total = src.len();

    assert!(w > 0 && total.is_multiple_of(w * 4));

    let bands = total / (w * 4);
    assert!(dst.len() >= bands * w);

    let src_ptr = src.as_ptr();
    let dst_ptr = dst.as_mut_ptr();
    let mut base = 0;

    for band in 0..bands {
        unsafe { pack_n_band::<4>(src_ptr, dst_ptr, w, band * w * 4, base) };
        base += w;
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
}

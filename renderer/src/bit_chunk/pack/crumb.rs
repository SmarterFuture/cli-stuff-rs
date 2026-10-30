use std::{array::from_fn, hint::unreachable_unchecked, slice::from_raw_parts};

use crate::bit_chunk::unchecked;

/// broadcasts lower 32 bits of u64 which holds 8 nibbles to full u64, so each nibble now occupies
/// top bits of byte
#[inline(always)]
fn broadcast_n(a: u64) -> u64 {
    let y = (a | (a << 16)) & 0x0000_FFFF_0000_FFFF;
    let z = (y | (y << 8)) & 0x00FF_00FF_00FF_00FF;
    (z & 0x00F0_00F0_00F0_00F0) | ((z << 12) & 0xF000_F000_F000_F000)
}

#[inline(always)]
unsafe fn pack_c_band<const R: usize>(
    src_ptr: *const u64,
    dst_ptr: *mut u64,
    w: usize,
    y: usize,
    base: usize,
) {
    const { assert!(R >= 1 && R <= 4) }

    let row_ptrs: [_; R] = unsafe { from_fn(|r| src_ptr.add(y + r * w)) };

    for x in 0..w {
        let mut lval = 0u64;
        let mut rval = 0u64;
        for (r, &row_p) in row_ptrs.iter().enumerate() {
            let raw = unchecked!(*row_p[x]).to_be();

            let val = (raw | (raw << 1)) & 0xCCCC_CCCC_CCCC_CCCC;
            match r {
                0 => lval = val,
                1 => lval |= val >> 2,
                2 => rval = val,
                3 => rval |= val >> 2,
                _ => unsafe { unreachable_unchecked() },
            }
        }
        let l = broadcast_n(lval >> 32) | broadcast_n(rval >> 32) >> 4;
        let r = broadcast_n(lval & 0xFFFF_FFFF) | broadcast_n(rval & 0xFFFF_FFFF) >> 4;

        unchecked!(*dst_ptr[base + 2 * x] = l);
        unchecked!(*dst_ptr[base + 2 * x + 1] = r);
    }
}

/// pack operation for lanes of size 2
pub(super) fn pack_c<'a>(w: usize, src: &[u64], dst: &'a mut [u64]) -> &'a [u8] {
    assert!(w <= usize::MAX / 64);
    let total = src.len();
    assert!(w > 0 && total.is_multiple_of(w * 4));

    let bands = total / (w * 4);
    assert!(dst.len() >= bands * w * 2);

    let src_ptr = src.as_ptr();
    let dst_ptr = dst.as_mut_ptr();
    let mut base = 0;

    for band in 0..bands {
        unsafe { pack_c_band::<4>(src_ptr, dst_ptr, w, band * w * 4, base) };
        base += 2 * w;
    }

    let len = bands * w * 16;
    unsafe { from_raw_parts(dst_ptr as *const u8, len) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- hot path: 4-row full band ---

    #[test]
    fn pack_c_full_band_zeros() {
        let src = [0u64; 4];
        let mut dst = [0u64; 2];
        let out = pack_c(1, &src, &mut dst);
        assert_eq!(out, &[0u8; 16]);
        assert_eq!(dst, [0, 0]);
    }

    #[test]
    fn pack_c_full_band_ones() {
        // 4 rows × 0xFF cells → lval = rval = u64::MAX
        // l = broadcast(0xFFFFFFFF) | broadcast(0xFFFFFFFF) >> 4 = u64::MAX
        let src = [u64::MAX; 4];
        let mut dst = [0u64; 2];
        pack_c(1, &src, &mut dst);
        assert_eq!(dst, [u64::MAX, u64::MAX]);
    }

    #[test]
    fn pack_c_full_band_mixed() {
        // raw = 0x0303030303030303 → val = 0x0404040404040404
        // r0: lval = 0x04..; r1: lval |= 0x01.. → lval = 0x05..
        // broadcast_n(0x05050505) = 0x5000_5000_5000_5000
        // >> 4                  = 0x0500_0500_0500_0500
        // l = 0x5500_5500_5500_5500
        let src = [0x0303030303030303u64; 4];
        let mut dst = [0u64; 2];
        pack_c(1, &src, &mut dst);
        assert_eq!(dst, [0x5500_5500_5500_5500, 0x5500_5500_5500_5500]);
    }

    // --- multiple bands ---

    #[test]
    fn pack_c_two_full_bands() {
        let src = [u64::MAX; 8];
        let mut dst = [0u64; 4];
        let out = pack_c(1, &src, &mut dst);
        assert_eq!(out.len(), 32);
        assert_eq!(dst, [u64::MAX; 4]);
    }
}

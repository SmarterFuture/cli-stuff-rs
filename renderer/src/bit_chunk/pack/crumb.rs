use std::{array::from_fn, hint::unreachable_unchecked, slice::from_raw_parts};

use crate::bit_chunk::pack::unchecked;

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

    let row_ptrs: [*const u64; R] = unsafe { from_fn(|r| src_ptr.add(y + r * w)) };

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

#[cold]
#[inline(never)]
unsafe fn pack_c_band_cold<const R: usize>(
    src_ptr: *const u64,
    dst_ptr: *mut u64,
    w: usize,
    y: usize,
    base: usize,
) {
    unsafe { pack_c_band::<R>(src_ptr, dst_ptr, w, y, base) };
}

/// pack operation for lanes of size 2
pub(super) fn pack_c<'a>(w: usize, src: &[u64], dst: &'a mut [u64]) -> &'a [u8] {
    assert!(w <= usize::MAX / 64);
    let total = src.len();
    assert!(w > 0 && total.is_multiple_of(w));

    let rows = total / w;
    let full_bands = rows / 4;
    let rem = rows % 4;
    let bands = full_bands + usize::from(rem > 0);
    assert!(dst.len() >= bands * w * 2);

    let src_ptr = src.as_ptr();
    let dst_ptr = dst.as_mut_ptr();
    let mut base = 0;

    for band in 0..full_bands {
        unsafe { pack_c_band::<4>(src_ptr, dst_ptr, w, band * w * 4, base) };
        base += 2 * w;
    }

    match rem {
        0 => {}
        1 => unsafe { pack_c_band_cold::<1>(src_ptr, dst_ptr, w, full_bands * w * 4, base) },
        2 => unsafe { pack_c_band_cold::<2>(src_ptr, dst_ptr, w, full_bands * w * 4, base) },
        3 => unsafe { pack_c_band_cold::<3>(src_ptr, dst_ptr, w, full_bands * w * 4, base) },
        _ => unsafe { unreachable_unchecked() },
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

    // --- cold path: partial bands (rem 1..=3) ---

    #[test]
    fn pack_c_three_rows() {
        // lval = u64::MAX (r0|r1), rval = 0xCCCC... (r2 only)
        // l = broadcast_n(0xFFFFFFFF) | broadcast_n(0xCCCCCCCC)>>4
        //   = 0xF0F0… | 0x0C0C… = 0xFCFC…
        let src = [u64::MAX; 3];
        let mut dst = [0u64; 2];
        pack_c(1, &src, &mut dst);
        assert_eq!(dst, [0xFCFC_FCFC_FCFC_FCFC, 0xFCFC_FCFC_FCFC_FCFC]);
    }

    #[test]
    fn pack_c_two_rows() {
        // lval = u64::MAX, rval = 0
        // l = broadcast_n(0xFFFFFFFF) = 0xF0F0…
        let src = [u64::MAX; 2];
        let mut dst = [0u64; 2];
        pack_c(1, &src, &mut dst);
        assert_eq!(dst, [0xF0F0_F0F0_F0F0_F0F0, 0xF0F0_F0F0_F0F0_F0F0]);
    }

    #[test]
    fn pack_c_one_row() {
        // lval = 0xCCCC…, rval = 0
        // broadcast_n(0xCCCCCCCC) = 0xC0C0…
        let src = [u64::MAX; 1];
        let mut dst = [0u64; 2];
        pack_c(1, &src, &mut dst);
        assert_eq!(dst, [0xC0C0_C0C0_C0C0_C0C0, 0xC0C0_C0C0_C0C0_C0C0]);
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

    #[test]
    fn pack_c_band_plus_partial() {
        // 5 rows → 1 full band + rem 1
        let src = [u64::MAX; 5];
        let mut dst = [0u64; 4]; // bands = 2, w = 1 → 2 × 2 = 4 u64s
        let out = pack_c(1, &src, &mut dst);
        assert_eq!(out.len(), 32);
        assert_eq!(dst[0..2], [u64::MAX; 2]); // full band
        assert_eq!(dst[2..4], [0xC0C0_C0C0_C0C0_C0C0; 2]); // 1-row tail
    }

    // --- w > 1 ---

    #[test]
    fn pack_c_w2_full_band() {
        // 4 rows × w=2 → rows = 2, rem = 2
        // 2-row path: lval = u64::MAX, rval = 0 → per column = 0xF0F0…
        let src = [u64::MAX; 4]; // 2 rows of 2 cells
        let mut dst = [0u64; 4]; // 2 cols × 2 u64
        let out = pack_c(2, &src, &mut dst);
        assert_eq!(out.len(), 32);
        assert_eq!(dst, [0xF0F0_F0F0_F0F0_F0F0; 4]);
    }

    #[test]
    fn pack_c_returns_correct_len() {
        for rows in 1..=8 {
            let src = vec![0u64; rows];
            let bands = rows.div_ceil(4);
            let mut dst = vec![0u64; bands * 2];
            let out = pack_c(1, &src, &mut dst);
            assert_eq!(out.len(), bands * 16, "rows={rows}");
        }
    }

    // --- property: no panics, no OOB across shapes ---

    #[test]
    fn pack_c_shape_sweep() {
        for rows in 1..=12 {
            for w in 1..=6 {
                let src = vec![0x1234_5678_9ABC_DEF0u64; rows * w];
                let bands = rows.div_ceil(4);
                let mut dst = vec![0u64; bands * w * 2];
                let _ = pack_c(w, &src, &mut dst);
            }
        }
    }
}

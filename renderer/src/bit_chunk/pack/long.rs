#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    _mm256_loadu_si256, _mm256_movemask_epi8, _mm256_or_si256, _mm256_setr_epi8,
    _mm256_shuffle_epi8,
};
use std::{
    array::from_fn,
    slice::{from_raw_parts, from_raw_parts_mut},
};

#[cfg(target_arch = "x86_64")]
use crate::bit_chunk::pack::avx_helpers::loadbur_block;
use crate::bit_chunk::pack::bwl_scalar::pack_bwl_band;

/// 4 columns of 4 rows -> 4 bytes
#[inline]
#[cfg(target_endian = "little")]
#[target_feature(enable = "avx2")]
unsafe fn pack_l_4by4_avx2(rows_ptr: &[*const u64; 4], x: usize, dst_ptr: *mut u8) {
    unsafe {
        // #[rustfmt::skip]
        // let mask = _mm256_setr_epi8(
        //     0, 4, 1, 5, 2, 6, 3, 7,
        //     8, 12, 9, 13, 10, 14, 11, 15,
        //     0, 4, 1, 5, 2, 6, 3, 7,
        //     8, 12, 9, 13, 10, 14, 11, 15,
        // );

        #[rustfmt::skip]
        let mask = _mm256_setr_epi8(
            3, 7, 2, 6, 1, 5, 0, 4,
            11, 15, 10, 14, 9, 13, 8, 12,
            3, 7, 2, 6, 1, 5, 0, 4,
            11, 15, 10, 14, 9, 13, 8, 12,
        );

        let t = loadbur_block!(rows_ptr, x, 3, 2, 1);

        let u = _mm256_shuffle_epi8(t, mask);
        let m = _mm256_movemask_epi8(u) as u32;
        (dst_ptr as *mut u32).write_unaligned(m);
    }
}

/// pack operation for lanes of size 32 (long)
pub(super) fn pack_l<'a>(w: usize, src: &[u64], dst: &'a mut [u64]) -> &'a [u8] {
    assert!(w <= usize::MAX / 32);
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
        let rows: [*const u64; 4] = unsafe { from_fn(|r| src_ptr.add(band * step + r * w)) };

        #[cfg(all(target_arch = "x86_64", target_endian = "little"))]
        if avx2 && w >= 4 {
            let main = w & !3;
            for x in (0..main).step_by(4) {
                unsafe {
                    pack_l_4by4_avx2(&rows, x, dst_ptr.add(base + x * 4));
                }
            }

            unsafe { pack_l_4by4_avx2(&rows, w - 4, dst_ptr.add(base + step - 16)) };
        } else {
            unsafe { pack_bwl_band::<32, 4>(&rows, 0, dst_ptr.add(base), w) };
        }

        #[cfg(not(all(target_arch = "x86_64", target_endian = "little")))]
        unsafe {
            pack_bwl_band::<32, 4>(&rows, 0, dst_ptr.add(base), w)
        };

        base += step;
    }

    unsafe { from_raw_parts(dst.as_ptr() as *const u8, size) }
}

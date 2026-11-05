use std::hint::unreachable_unchecked;

use crate::bit_chunk::unchecked;

/// compresses sparse bytes according to masks and writes to dst_ptr with offset
macro_rules! compress_write {
    ($dst:expr, $off:expr, $acc:expr, $count:literal, [$($i:literal),+ $(,)?]) => {{
        let b = $acc.to_le_bytes();
        unsafe { ($dst as *mut [u8; $count]).add($off).write_unaligned([$(b[$i]),+]) }
    }};
}

#[inline(always)]
pub(super) unsafe fn pack_bwl_band<const N: usize, const R: usize>(
    rows_ptr: &[*const u64; 4],
    x: usize,
    dst_ptr: *mut u8,
    w: usize,
) {
    const { assert!(N == 32 || N == 16 || N == 8) }

    for x in x..w {
        let mut val = 0;

        for (r, &row_p) in rows_ptr.iter().enumerate() {
            val |= unchecked!(*row_p[x]) >> (r * 2);
        }

        let acc = val | (val << (N - 1));

        match N {
            8 => compress_write!(dst_ptr, x, acc, 4, [7, 5, 3, 1]),
            16 => compress_write!(dst_ptr, x, acc, 2, [7, 3]),
            32 => compress_write!(dst_ptr, x, acc, 1, [7]),
            _ => unsafe { unreachable_unchecked() },
        };
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     /// "10110010" -> u64: pixel `p` (0 = leftmost) sits at bit `63 - p*N`.
//     fn word<const N: usize>(pixels: &str) -> u64 {
//         assert!(pixels.len() <= 64 / N);
//         let mut v = 0u64;
//         for (p, c) in pixels.bytes().enumerate() {
//             if c == b'1' {
//                 v |= 1u64 << (63 - p * N);
//             }
//         }
//         v
//     }
//
//     /// Run pack_bwl against a pre-filled dst with one guard word; verify nothing
//     /// at or past the returned length was written.
//     fn run<const N: usize>(w: usize, src: &[u64]) -> Vec<u8> {
//         let bands = src.len() / (4 * w);
//         let len = bands * w * (32 / N);
//         let words = len.div_ceil(8);
//         let mut dst = vec![0xAAAA_AAAA_AAAA_AAAAu64; words + 1];
//         let out = pack_bwl::<N>(w, src, &mut dst[..words]).to_vec();
//
//         assert_eq!(out.len(), len, "N={N}: returned length");
//
//         let bytes: Vec<u8> = dst.iter().flat_map(|v| v.to_ne_bytes()).collect();
//         assert!(
//             bytes[len..].iter().all(|&b| b == 0xAA),
//             "N={N}: wrote past the end (len={len})",
//         );
//         out
//     }
//
//     fn test<const N: usize>(rows: &[&str], expected: &[u8]) {
//         assert_eq!(rows.len(), 4);
//         let src: Vec<u64> = rows.iter().map(|r| word::<N>(r)).collect();
//         assert_eq!(run::<N>(1, &src), expected, "N={N}");
//     }
//
//     // ---------- Hand-computed ----------
//
//     #[test]
//     fn test_n8() {
//         // pixels 0,2,3,6 set in every row:
//         //   byte 0 (p 0,1): left  -> 0xAA
//         //   byte 1 (p 2,3): both  -> 0xFF
//         //   byte 2 (p 4,5): none  -> 0x00
//         //   byte 3 (p 6,7): left  -> 0xAA
//         test::<8>(&["10110010"; 4], &[0xAA, 0xFF, 0x00, 0xAA]);
//     }
//
//     #[test]
//     fn test_n16() {
//         // p0@63, p1@47, p2@31, p3@15. Row 0 "1011" sets p0,p2,p3; etc.
//         test::<16>(&["1011", "0100", "1101", "0010"], &[0x9C, 0xC6]);
//     }
//
//     #[test]
//     fn test_n32() {
//         test::<32>(&["10", "01", "11", "00"], &[0x9C]);
//     }
//
//     // ---------- All set / all clear ----------
//
//     #[test]
//     fn all_set_and_all_clear() {
//         test::<8>(&["11111111"; 4], &[0xFF; 4]);
//         test::<16>(&["1111"; 4], &[0xFF; 2]);
//         test::<32>(&["11"; 4], &[0xFF]);
//         test::<8>(&["00000000"; 4], &[0; 4]);
//         test::<16>(&["0000"; 4], &[0; 2]);
//         test::<32>(&["00"; 4], &[0]);
//     }
//
//     #[test]
//     fn left_pixel_is_the_high_bit() {
//         // Pixel 0 in every row -> bits 7,5,3,1 of byte 0 only.
//         test::<8>(&["10000000"; 4], &[0xAA, 0, 0, 0]);
//         test::<8>(&["01000000"; 4], &[0x55, 0, 0, 0]);
//         test::<16>(&["1000"; 4], &[0xAA, 0]);
//         test::<16>(&["0100"; 4], &[0x55, 0]);
//         test::<32>(&["10"; 4], &[0xAA]);
//         test::<32>(&["01"; 4], &[0x55]);
//     }
//
//     // ---------- Single-pixel sweep ----------
//
//     fn single_pixel_sweep<const N: usize>() {
//         let bpc = 32 / N;
//         for w in 1..=5usize {
//             for bands in 1..=2usize {
//                 for row in 0..bands * 4 {
//                     for x in 0..w {
//                         for p in 0..64 / N {
//                             let mut src = vec![0u64; bands * 4 * w];
//                             src[row * w + x] = 1u64 << (63 - p * N);
//
//                             let mut expected = vec![0u8; bands * w * bpc];
//                             expected[((row / 4) * w + x) * bpc + p / 2] =
//                                 0x80 >> (2 * (row % 4) + (p & 1));
//
//                             assert_eq!(
//                                 run::<N>(w, &src),
//                                 expected,
//                                 "N={N} w={w} bands={bands} row={row} x={x} p={p}"
//                             );
//                         }
//                     }
//                 }
//             }
//         }
//     }
//
//     #[test]
//     fn sweep_n8() {
//         single_pixel_sweep::<8>()
//     }
//     #[test]
//     fn sweep_n16() {
//         single_pixel_sweep::<16>()
//     }
//     #[test]
//     fn sweep_n32() {
//         single_pixel_sweep::<32>()
//     }
//
//     // ---------- Panic paths ----------
//
//     #[test]
//     #[should_panic]
//     fn total_not_multiple_of_4w_panics() {
//         pack_bwl::<8>(1, &[0u64; 3], &mut [0u64; 4]); // 3 not a multiple of 4
//     }
//
//     #[test]
//     #[should_panic]
//     fn w_zero_panics() {
//         pack_bwl::<8>(0, &[0u64; 4], &mut [0u64; 4]);
//     }
//
//     #[test]
//     #[should_panic]
//     fn dst_too_small_panics() {
//         pack_bwl::<8>(1, &[0u64; 4], &mut []); // need 4 bytes, have 0
//     }
// }

use std::hint::unreachable_unchecked;

use crate::bit_chunk::pack::{
    byte_word_long::{pack_b, pack_bwl},
    crumb::pack_c,
    nibble::pack_n,
    quad::pack_q,
};

mod avx_helpers;
mod byte_avx;
mod byte_word_long;
mod crumb;
mod long_avx;
mod nibble;
mod quad;
mod word_avx;

/// pack operation dispatching to crumb/nibble/byte/word/long/quad impls by lane size N
/// braids `src` 2 columns at a time down all rows, writing packed output into `dst` and
/// returning it as `&[u8]` aliasing the same storage (no copy, length = bytes actually written)
#[inline]
pub fn pack<'a, const N: usize>(w: usize, src: &mut [u64], dst: &'a mut [u64]) -> &'a [u8] {
    const { assert!(N.is_power_of_two() && N >= 2 && N <= 64) }
    assert!(w <= usize::MAX / 64);

    match N {
        2 => pack_c(w, src, dst),
        4 => pack_n(w, src, dst),
        8 => pack_b(w, src, dst),
        16 => pack_bwl::<16>(w, src, dst),
        32 => pack_bwl::<32>(w, src, dst),
        64 => pack_q(w, src, dst),
        _ => unsafe { unreachable_unchecked() },
    }
}

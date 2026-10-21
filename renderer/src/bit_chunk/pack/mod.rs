use std::hint::unreachable_unchecked;

use crate::bit_chunk::pack::{
    crumb::pack_c,
    nibble::pack_n,
    pack_t::{pack_bwl, pack_q},
};

mod crumb;
mod nibble;
mod pack_t;

macro_rules! unchecked {
    ($src:ident[$idx:expr] $op:tt $rest:expr) => {
        unsafe {
            debug_assert!($idx < $src.len());
            *$src.get_unchecked_mut($idx) $op $rest
        }
    };
    ($src:ident[$idx:expr]) => {
        unsafe {
            debug_assert!($idx < $src.len());
            *$src.get_unchecked($idx)
        }
    };
    (*$ptr:ident[$idx:expr]) => {
        unsafe { *$ptr.add($idx) }
    };
    (*$ptr:ident[$idx:expr] $op:tt $rest:expr) => {
        unsafe { *$ptr.add($idx) $op $rest}
    };
}

pub(super) use unchecked;

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
        8 => pack_bwl::<8>(w, src, dst),
        16 => pack_bwl::<16>(w, src, dst),
        32 => pack_bwl::<32>(w, src, dst),
        64 => pack_q(w, src, dst),
        _ => unsafe { unreachable_unchecked() },
    }
}

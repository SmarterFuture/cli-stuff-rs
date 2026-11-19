use std::hint::unreachable_unchecked;

use crate::bit_chunk::downsize::{downsize_generic::downsize_3to2, downsize_pow2::downsize_pow2};

// FIXME: remove this pub, for asm only
pub mod downsize_generic;
mod downsize_pow2;

pub fn downsize(n: usize, w: usize, src: &mut [u64]) -> (&mut [u64], &mut [u64]) {
    assert!(n <= 64);

    match n {
        2 => downsize_pow2::<2>(w, src),
        4 => downsize_pow2::<4>(w, src),
        8 => downsize_pow2::<8>(w, src),
        16 => downsize_pow2::<16>(w, src),
        32 => downsize_pow2::<32>(w, src),
        64 => downsize_pow2::<64>(w, src),
        3 => downsize_3to2(w, src),
        x if x < 64 => todo!("generic impl"),
        _ => unsafe { unreachable_unchecked() },
    }
}

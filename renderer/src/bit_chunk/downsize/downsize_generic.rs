use std::{
    arch::x86_64::{
        __m256i, _mm256_and_si256, _mm256_extract_epi64, _mm256_loadu_si256, _mm256_or_si256,
        _mm256_permute4x64_epi64, _mm256_setr_epi64x, _mm256_slli_epi64, _mm256_sllv_epi64,
        _mm256_srli_epi64, _mm256_xor_si256, _pdep_u64, _pext_u64,
    },
    array::from_fn,
};

use crate::bit_chunk::unchecked;

/// lane patter 100
const HI3: u64 = 0x9249_2492_4924_9248;
/// packed deposit top aligned for 3 wide downsize
const PDT3: u64 = 0xAAAA_AAAA_AA80_0000;
/// packed deposit bottom aligned for 3 wide downsize
const PDB3: u64 = 0x0000_0AAA_AAAA_AAA8;
/// packed deposit for 3 wide downsize masks
const PD3M: [u64; 3] = [PDT3, PDT3, PDB3];
/// packed deposit all for 3 wide downsize
const PDA3: u64 = 0xAAAA_AAAA_AAAA_AAAA;

/// HI3 but with bottom bit also set
const HI3A: u64 = HI3 | 1;

/// -> (set, carry)
#[inline(always)]
const fn halfish_adder3(a: u64, b: u64, c: u64) -> (u64, u64) {
    let p = a ^ b;
    let q = p ^ c;
    (q, (a & b) | (p & c))
}

/// -> lane pattern x00
/// where x = 1 if majority is 1, and 0 if majority is 0
/// where set + 2 * carry = true popcount
#[inline(always)]
const fn cmp_maj3(set: u64, carry: u64) -> u64 {
    let (c2, c1, c0) = (carry, carry << 1, carry << 2);
    let (s2, s1, s0) = (set, set << 1, set << 2);

    let a = c2 | c1;
    let b = c2 & c1;
    let maj = b | (a & c0);
    let cor = a | c0;
    let cnd = b & c0;

    let sor = s2 | s1 | s0;
    let snd = s2 & s1 & s0;

    (sor | cnd) & (maj | snd) & cor & HI3
}

#[inline(always)]
fn combine1by3(
    row_ptrs: &[*const u64; 3],
    base: usize,
    off: usize,
    set: &mut [u64],
    carry: &mut [u64],
) {
    let mut rs = [0; 3];
    for (j, &row) in row_ptrs.iter().enumerate() {
        rs[j] = unchecked!(*row[base + off]);
    }

    let (s, c) = halfish_adder3(rs[0], rs[1], rs[2]);

    // this should still be in registers so no stack spill or RMW pattern
    set[off] |= s >> off;
    carry[off] |= c >> off;

    set[off + 1] = s << (63 - off);
    carry[off + 1] = c << (63 - off);
}

/// N in {1, 2, 3}
#[inline]
#[target_feature(enable = "bmi2")]
unsafe fn downsize_3to2_nby3<const N: usize>(
    row_ptrs: &[*const u64; 3],
    x: usize,
    dst_ptr: *mut u64,
    wx: usize,
) {
    const { assert!(N > 0 && N <= 3) };

    // in like very specific case we over allocate by like 2, which is fully acceptable, bc fixing
    // it is pain
    // the llvm should be smart enough to drop those
    let mut set = [0; 4];
    let mut carry = [0; 4];

    let mut col = [0; N];

    // if llvm wont be too sloppy it will unroll both loops inner and this one)
    for k in 0..N {
        combine1by3(row_ptrs, x, k, &mut set, &mut carry);

        let cmp = cmp_maj3(set[k], carry[k]);

        // i really dont like this (use of questionable pext/pdep) (hello zen1/2)
        // FUTURE: make it a feature flag, pext/pdep will be on by default
        let tmp = _pext_u64(cmp, HI3);
        col[k] = _pdep_u64(tmp, PD3M[k]);
    }

    if N != 1 {
        let o1 = col[0] | (col[1] >> 42);
        unchecked!(*dst_ptr[wx] = o1);
    }

    match N {
        1 => {
            let o1 = col[0];
            unchecked!(*dst_ptr[wx] = o1);
        }
        2 => {
            let l42 = cmp_maj3(set[2], carry[2]) >> 20;
            let o2 = col[1] << 22 | l42;
            unchecked!(*dst_ptr[wx + 1] = o2);
        }
        3 => {
            // last straddled lane
            let l63 = cmp_maj3(set[3], carry[3]) >> 62;
            let o2 = (col[1] << 22) | col[2] | l63;
            unchecked!(*dst_ptr[wx + 1] = o2);
        }
        _ => {}
    }
}

/// shift by 1 and 2 across the lower 192 bits (lanes 0, 1, 2).
/// lane 3 contains garbage and should be masked out
/// -> (shl 1, shl 2)
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn shl1_2_192(v: __m256i) -> (__m256i, __m256i) {
    let next = _mm256_permute4x64_epi64::<0x39>(v);
    let v1 = _mm256_or_si256(_mm256_slli_epi64::<1>(v), _mm256_srli_epi64::<63>(next));
    let v2 = _mm256_or_si256(_mm256_slli_epi64::<2>(v), _mm256_srli_epi64::<62>(next));
    (v1, v2)
}

/// requires at least one quad of valid memory past its processed block
#[inline]
#[target_feature(enable = "avx2,bmi2")]
unsafe fn downsize_3to2_3by3_avx2(
    row_ptrs: &[*const u64; 3],
    x: usize,
    dst_ptr: *mut u64,
    wx: usize,
) {
    let mask = _mm256_setr_epi64x(0, 2, 1, 0);

    let rs: [_; 3] = unsafe { from_fn(|i| _mm256_loadu_si256(row_ptrs[i].add(x).cast::<_>())) };

    let p = _mm256_xor_si256(rs[0], rs[1]);

    let s2 = _mm256_xor_si256(rs[2], p);
    let c2 = _mm256_or_si256(_mm256_and_si256(rs[0], rs[1]), _mm256_and_si256(p, rs[2]));

    // same as cmp_maj3 but in avx2
    let (s1, s0) = unsafe { shl1_2_192(s2) };
    let (c1, c0) = unsafe { shl1_2_192(c2) };

    let a = _mm256_or_si256(c2, c1);
    let b = _mm256_and_si256(c2, c1);
    let maj = _mm256_or_si256(_mm256_and_si256(a, c0), b);
    let cor = _mm256_or_si256(a, c0);
    let cnd = _mm256_and_si256(c0, b);

    let sor = _mm256_or_si256(s2, _mm256_or_si256(s1, s0));
    let snd = _mm256_and_si256(s2, _mm256_and_si256(s1, s0));

    let a = _mm256_or_si256(sor, cnd);
    let b = _mm256_or_si256(maj, snd);
    let a = _mm256_and_si256(a, b);
    let cmp = _mm256_and_si256(a, cor);
    let cmp = _mm256_sllv_epi64(cmp, mask);

    // extract and pext -> pdep -> store
    let a = _mm256_extract_epi64::<0>(cmp) as _;
    let b = _mm256_extract_epi64::<1>(cmp) as _;
    let c = _mm256_extract_epi64::<2>(cmp) as _;

    let a = _pext_u64(a, HI3A);
    let b = _pext_u64(b, HI3A);
    let c = _pext_u64(c, HI3A);

    let a = _pdep_u64(a, PDA3) << 20;
    let b = _pdep_u64(b, PDA3);
    let c = _pdep_u64(c, PDA3) >> 2;

    let o1 = a | (b >> 24);
    let o2 = (b << 40) | c;

    unchecked!(*dst_ptr[wx] = o1);
    unchecked!(*dst_ptr[wx + 1] = o2);
}

/// due to 256-bit unaligned loads on the avx2 hot path, `src` must include at least one padding
/// `u64` past the `rows * w` data elements (i.e. `src.len() == rows * w + 1`).
pub fn downsize_3to2(w: usize, src: &mut [u64]) -> (&mut [u64], &mut [u64]) {
    let total = src.len();

    assert!(w > 0 && total > 1 && (total - 1).is_multiple_of(w));

    let rows = (total - 1) / w;
    let bands = rows / 3;

    assert!(bands > 0);

    let rem = rows % 3;

    let band_rem = w % 3;
    let full = w / 3;

    let all = bands + (rem == 2) as usize;

    let ow = 2 * full + band_rem;

    // they are aliased, but there should never be write before read so it should be fine
    let dst_ptr = src.as_mut_ptr();
    let src_ptr = dst_ptr.cast_const();

    #[cfg(target_arch = "x86_64")]
    let avx2 = is_x86_feature_detected!("avx2");

    for y in 0..all {
        let band = y * w * 3;
        let wx = y * ow;

        let row_ptrs: [_; 3] = unsafe { from_fn(|r| src_ptr.add(band + r * w)) };

        for i in 0..full {
            if avx2 {
                unsafe { downsize_3to2_3by3_avx2(&row_ptrs, 3 * i, dst_ptr, wx + 2 * i) };
            } else {
                unsafe { downsize_3to2_nby3::<3>(&row_ptrs, 3 * i, dst_ptr, wx + 2 * i) };
            }
        }

        let wx = wx + 2 * full;
        match band_rem {
            1 => unsafe {
                downsize_3to2_nby3::<1>(&row_ptrs, 3 * full, dst_ptr, wx);
            },
            2 => unsafe {
                downsize_3to2_nby3::<2>(&row_ptrs, 3 * full, dst_ptr, wx);
            },
            _ => {}
        }
    }

    // FIXME: we are missing like the last incomplete band...

    if rem == 1 {
        unsafe { dst_ptr.add(ow * bands).write_bytes(0, ow) };
    }

    unsafe { src.split_at_mut_unchecked(ow * rows.div_ceil(3)) }
}

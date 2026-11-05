#[cfg(target_arch = "x86_64")]
macro_rules! load_block {
    ($rows:ident, $x:expr, $s1:literal, $s2:literal, $s3:literal) => {{
        let __off = $x;
        let a0 = _mm256_loadu_si256($rows[0].add(__off) as _);
        let a1 = _mm256_srli_epi64::<$s1>(_mm256_loadu_si256($rows[1].add(__off) as _));
        let a2 = _mm256_srli_epi64::<$s2>(_mm256_loadu_si256($rows[2].add(__off) as _));
        let a3 = _mm256_srli_epi64::<$s3>(_mm256_loadu_si256($rows[3].add(__off) as _));
        _mm256_or_si256(_mm256_or_si256(a0, a1), _mm256_or_si256(a2, a3))
    }};
}

/// SAFETY: strictly ensures that $x + $s1/2/3 is valid address
#[cfg(target_arch = "x86_64")]
#[cfg(target_endian = "little")]
macro_rules! loadbur_block {
    ($rows:ident, $x:expr, $s0:literal, $s1:literal, $s2:literal) => {{
        let __off = $x;
        let a0 = _mm256_loadu_si256($rows[0].add(__off).cast::<u8>().add($s0) as _);
        let a1 = _mm256_loadu_si256($rows[1].add(__off).cast::<u8>().add($s1) as _);
        let a2 = _mm256_loadu_si256($rows[2].add(__off).cast::<u8>().add($s2) as _);
        let a3 = _mm256_loadu_si256($rows[3].add(__off) as _);
        _mm256_or_si256(_mm256_or_si256(a0, a1), _mm256_or_si256(a2, a3))
    }};
}

pub(super) use load_block;
pub(super) use loadbur_block;

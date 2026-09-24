use std::{
    arch::x86_64::{_pdep_u64, _pext_u64},
    mem::transmute,
};

#[inline]
pub const fn lane_lsb_mask<const N: usize>() -> u64 {
    const { assert!(N.is_power_of_two()) }
    u64::MAX / ((1u64 << N) - 1)
}

#[inline]
pub fn cmp_ge<const N: usize>(x: u64, val: u64) -> u64 {
    const { assert!(N.is_power_of_two()) }

    let lsb = lane_lsb_mask::<N>();
    let val_per_lane = val.wrapping_mul(lsb);

    let sub = x.wrapping_sub(val_per_lane);
    let carry = (sub >> (N - 1)) & lsb;
    println!("carry {:x}", carry);

    carry ^ lsb
}

#[inline]
const fn lane_popcount<const N: usize>(x: u64) -> u64 {
    const { assert!((N - 1) & N == 0) }

    const M3: u64 = 0x3333_3333_3333_3333;
    const M5: u64 = 0x5555_5555_5555_5555;
    const MF: u64 = 0x0F0F_0F0F_0F0F_0F0F;
    const MFF: u64 = 0x00FF_00FF_00FF_00FF;

    match N {
        2 => ((x >> 1) & M5) + (x & M5),
        4 => {
            let v = lane_popcount::<2>(x);
            (v & M3) + ((v >> 2) & M3)
        }
        8 => {
            let v = lane_popcount::<4>(x);
            (v & MF) + ((v >> 4) & MF)
        }
        16 => {
            let v = lane_popcount::<8>(x);
            (v & MFF) + ((v >> 8) & MFF)
        }
        32 => unsafe {
            let v = transmute::<u64, [u32; 2]>(x);
            v[0].count_ones() as u64 | (v[1].count_ones() as u64) << 32
        },
        64 => x.count_ones() as u64,
        _ => unreachable!(),
    }
}

#[inline]
const fn saturating_add<const N: usize>(a: u64, b: u64) -> u64 {
    const { assert!(N == 2 || N == 4) }

    let mask = match N {
        2 => 0x0404_0404_0404_0404,
        4 => 0x1010_1010_1010_1010,
        _ => unreachable!(),
    };

    let sum = a.wrapping_add(b);
    let carry = (a ^ b ^ sum) & mask;
    let overflow = carry - (carry >> N);

    (sum & !mask) | overflow
}

#[inline]
fn pext(x: u64, m: u64) -> u64 {
    unsafe { _pext_u64(x, m) }
}

#[inline]
fn pdep(x: u64, m: u64) -> u64 {
    unsafe { _pdep_u64(x, m) }
}

#[inline]
fn combine<const N: usize>(acc: &mut u64, val: u64) {
    const { assert!(N.is_power_of_two() && N > 1) }

    let pop = lane_popcount::<N>(val);

    match N {
        2 | 4 => *acc = saturating_add::<N>(*acc, pop),
        _ => *acc = acc.wrapping_add(pop),
    };
}

pub fn downsize<const LEN: usize, const W: usize, const N: usize, const OUT: usize>(
    mut data: [u64; LEN],
) -> [u64; OUT] {
    const { assert!(N.is_power_of_two() && N > 0) }
    const { assert!(LEN.is_multiple_of(W) && LEN.is_multiple_of(N)) }
    const { assert!(OUT == LEN / N) }

    let mut out = [0; OUT];

    for i in 0..LEN {
        let line = i / W;
        let local_line = line % N;
        let pos = i % W;
        let local_pos = ( line - local_line ) * W + pos;

        if local_line == 0 {
            let b = &mut data[i];
            *b = lane_popcount::<N>(*b);
        } else {
            let val = data[i];
            combine::<N>(&mut data[local_pos], val);
        }
        
        if local_line == N - 1 {
            let out_i = pos * (line / N) ;
            let offset = pos % N;

            let raw = &data[local_pos];
            println!("{}", 1 << (N - 1));
            let cmp = cmp_ge::<N>(*raw, 1 << (N - 1));
            println!("{:x}", cmp);
            out[out_i] = pext(cmp, lane_lsb_mask::<N>()) << (offset << N);
            println!("{:x}", out[out_i]);
        }
    }
    
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downsize_majority_ones() {
        const LEN: usize = 4;
        const W: usize = 1;
        const N: usize = 2;
        const OUT: usize = LEN / N; // = 2

        let input: [u64; LEN] = [
            0b11,
            0b10,
            0b01,
            0b10,
        ];

        let expected: [u64; OUT] = [0b01, 0b01];

        let output = downsize::<LEN, W, N, OUT>(input);
        assert_eq!(output, expected);
    }
}



use std::ptr::copy_nonoverlapping;

#[inline(always)]
fn read_word(data: &[u8], idx: usize, offset: u8) -> u64 {
    let ptr = unsafe { data.as_ptr().add(idx) };
    let mut block: u128 = 0;

    let available = data.len().saturating_sub(idx);
    let to_load = available.min(9);

    unsafe {
        copy_nonoverlapping(ptr, &mut block as *mut u128 as *mut u8, to_load);
    }

    (block.to_be() >> (64 - offset)) as u64
}

pub fn align<const W: usize>(data: &[u8]) -> Vec<u64> {
    let total_bits = data.len().checked_mul(8).expect("data too large");

    let words = W.div_ceil(64);
    let lines = total_bits / W;
    let mut out = vec![0; lines * words];

    let mut row_bit_idx: usize = 0;
    let mut roff = 0;
    let mut pos = 0;

    let trail = words * 64 - W;
    let overf = trail.div_ceil(8);
    let cerr = (W % 8) as u8;

    let mask_n = 0xFFFFFFFFFFFFFFFF << trail;
    let mask3 = 0x7;

    for word in out.iter_mut() {
        *word = read_word(data, pos, roff);

        row_bit_idx += 64;
        pos += 8;

        if row_bit_idx >= W {
            *word &= mask_n;

            roff += cerr;
            pos -= (0 < roff) as usize + overf;
            roff &= mask3;

            row_bit_idx = 0
        }
    }
    out
}

// pub struct BitChunkIter {
//     w
// }

#[cfg(test)]
mod tests {
    use super::align;

    #[test]
    fn align_simple() {
        let arr: [u8; 12] = [0xFF; 12];

        let out = align::<96>(&arr);
        let expected = vec![0xFFFFFFFFFFFFFFFF, 0xFFFFFFFF00000000];
        assert_eq!(out, expected);
    }

    #[test]
    fn align_two_lines() {
        let arr: [u8; 23] = [
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xF0,
        ];

        let out = align::<90>(&arr);
        let expected = vec![
            0xFFFFFFFFFFFFFFFF,
            0xFFFFFFC000000000,
            0xFFFFFFFFFFFFFFFF,
            0xFFFFFFC000000000,
        ];
        assert_eq!(out, expected);
    }

    #[test]
    fn align_64() {
        let arr: [u8; 16] = [0xFF; 16];

        let out = align::<64>(&arr);
        let expected = vec![0xFFFFFFFFFFFFFFFF, 0xFFFFFFFFFFFFFFFF];
        assert_eq!(out, expected);
    }

    #[test]
    fn align_random() {
        let arr: [u8; 8] = [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x78, 0x90];

        let out = align::<32>(&arr);
        let expected = vec![0xA1B2C3D400000000, 0xE5F6789000000000];
        assert_eq!(out, expected);
    }
}

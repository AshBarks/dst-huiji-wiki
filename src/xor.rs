const BLOCK_SIZE: usize = 8;
const XOR_KEY: [u8; BLOCK_SIZE] = [141, 142, 143, 144, 145, 146, 147, 148];
const PERMUTATION: [usize; BLOCK_SIZE] = [5, 3, 6, 7, 4, 2, 0, 1];

fn xor_process(data: &[u8], encrypt: bool) -> Vec<u8> {
    if !encrypt && data.len() >= 2 && data[0] == b'P' && data[1] == b'K' {
        return data.to_vec();
    }
    if data.len() < 16 {
        return data.to_vec();
    }

    let mut output = Vec::with_capacity(data.len());
    let data_len = data.len();
    let mut pos = 0;

    while pos + 8 < data_len {
        let mut block = [0u8; BLOCK_SIZE];
        for n in 0..BLOCK_SIZE {
            let perm_idx = PERMUTATION[n];
            if encrypt {
                block[perm_idx] = data[pos + n] ^ XOR_KEY[n];
            } else {
                block[n] = data[pos + perm_idx] ^ XOR_KEY[n];
            }
        }
        output.extend_from_slice(&block);
        pos += 8;
    }

    if pos < data_len {
        output.extend_from_slice(&data[pos..]);
    }

    output
}

pub fn xor_decrypt(data: &[u8]) -> Vec<u8> {
    if data.len() < 2 {
        return data.to_vec();
    }
    xor_process(data, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pk_detection() {
        let pk_data = b"PK\x03\x04test_data_here_padding_to_16bytes";
        let result = xor_decrypt(pk_data);
        assert_eq!(result, pk_data.to_vec());
    }

    #[test]
    fn short_data_passthrough() {
        let short = vec![1u8, 2, 3];
        let result = xor_decrypt(&short);
        assert_eq!(result, short);
    }

    #[test]
    fn decrypt_block_logic() {
        let input = vec![
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let decrypted = xor_decrypt(&input);
        assert_eq!(decrypted.len(), 8 + 8);
        for n in 0..BLOCK_SIZE {
            assert_eq!(decrypted[n], input[PERMUTATION[n]] ^ XOR_KEY[n]);
        }
        for n in BLOCK_SIZE..16 {
            assert_eq!(decrypted[n], input[n]);
        }
    }
}

const BLOCK_SIZE: usize = 8;
const XOR_KEY: [u8; BLOCK_SIZE] = [141, 142, 143, 144, 145, 146, 147, 148];
const PERMUTATION: [usize; BLOCK_SIZE] = [5, 3, 6, 7, 4, 2, 0, 1];

fn xor_cipher_block(input: &[u8], encrypt: bool) -> Vec<u8> {
    if input.len() <= BLOCK_SIZE {
        return input.to_vec();
    }
    let mut output = vec![0u8; BLOCK_SIZE];
    for n in 0..BLOCK_SIZE {
        let perm_idx = PERMUTATION[n];
        if encrypt {
            output[perm_idx] = input[n] ^ XOR_KEY[n];
        } else {
            output[n] = input[perm_idx] ^ XOR_KEY[n];
        }
    }
    output
}

pub fn xor_decrypt(data: &[u8]) -> Vec<u8> {
    if data.len() < 2 {
        return data.to_vec();
    }
    if data[0] == b'P' && data[1] == b'K' {
        return data.to_vec();
    }
    if data.len() < 16 {
        return data.to_vec();
    }

    let mut output = Vec::new();
    let data_len = data.len();
    let mut cursor = 16;

    let mut window = data[0..16].to_vec();
    output.extend(xor_cipher_block(&window, false));

    loop {
        let second_half = window[BLOCK_SIZE..].to_vec();
        let fresh_len = std::cmp::min(BLOCK_SIZE, data_len - cursor);
        let fresh = if fresh_len > 0 {
            data[cursor..cursor + fresh_len].to_vec()
        } else {
            Vec::new()
        };
        cursor += fresh_len;

        window = second_half;
        window.extend_from_slice(&fresh);

        if window.is_empty() {
            break;
        }

        if window.len() > BLOCK_SIZE {
            output.extend(xor_cipher_block(&window, false));
        } else {
            output.extend_from_slice(&window);
            break;
        }
    }

    output
}

pub fn xor_encrypt(data: &[u8]) -> Vec<u8> {
    if data.len() < 16 {
        return data.to_vec();
    }

    let mut output = Vec::new();
    let data_len = data.len();
    let mut cursor = 16;

    let mut window = data[0..16].to_vec();
    output.extend(xor_cipher_block(&window, true));

    loop {
        let second_half = window[BLOCK_SIZE..].to_vec();
        let fresh_len = std::cmp::min(BLOCK_SIZE, data_len - cursor);
        let fresh = if fresh_len > 0 {
            data[cursor..cursor + fresh_len].to_vec()
        } else {
            Vec::new()
        };
        cursor += fresh_len;

        window = second_half;
        window.extend_from_slice(&fresh);

        if window.is_empty() {
            break;
        }

        if window.len() > BLOCK_SIZE {
            output.extend(xor_cipher_block(&window, true));
        } else {
            output.extend_from_slice(&window);
            break;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_roundtrip() {
        let original = vec![0u8; 32];
        let encrypted = xor_encrypt(&original);
        assert_ne!(encrypted, original);
        let decrypted = xor_decrypt(&encrypted);
        assert_eq!(decrypted, original);
    }

    #[test]
    fn xor_roundtrip_random() {
        let original: Vec<u8> = (0..100).map(|i| (i * 7 + 13) as u8).collect();
        let encrypted = xor_encrypt(&original);
        let decrypted = xor_decrypt(&encrypted);
        assert_eq!(decrypted, original);
    }

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

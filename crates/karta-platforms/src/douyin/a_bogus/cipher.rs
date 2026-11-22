/// Standard RC4 stream cipher operating on raw byte slices.
pub(super) fn rc4_encrypt(plaintext: &[u8], key: &[u8]) -> Vec<u8> {
    let mut state = (0..=255).collect::<Vec<u8>>();
    let mut j = 0usize;

    for i in 0..256 {
        j = (j + state[i] as usize + key[i % key.len()] as usize) % 256;
        state.swap(i, j);
    }

    let mut i = 0usize;
    j = 0;
    let mut cipher = Vec::with_capacity(plaintext.len());

    for &byte in plaintext {
        i = (i + 1) % 256;
        j = (j + state[i] as usize) % 256;
        state.swap(i, j);

        let t = (state[i] as usize + state[j] as usize) % 256;
        cipher.push(state[t] ^ byte);
    }

    cipher
}

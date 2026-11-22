const SM3_IV: [u32; 8] = [
    0x7380166f, 0x4914b2b9, 0x172442d7, 0xda8a0600, 0xa96f30bc, 0x163138aa, 0xe38dee4d, 0xb0fb0e4e,
];

pub(crate) fn sm3_hash(data: &[u8]) -> [u8; 32] {
    let mut h = Sm3::new();
    h.sum(Some(data))
}

pub(super) fn sm3_double(data: &[u8]) -> [u8; 32] {
    let inner = sm3_hash(data);
    sm3_hash(&inner)
}

struct Sm3 {
    reg: [u32; 8],
    chunk: Vec<u8>,
    size: usize,
}

impl Sm3 {
    fn new() -> Self {
        Self {
            reg: SM3_IV,
            chunk: Vec::new(),
            size: 0,
        }
    }

    fn reset(&mut self) {
        self.reg = SM3_IV;
        self.chunk.clear();
        self.size = 0;
    }

    fn write_bytes(&mut self, data: &[u8]) {
        self.size += data.len();
        let remaining = 64 - self.chunk.len();

        if data.len() < remaining {
            self.chunk.extend_from_slice(data);
            return;
        }

        self.chunk.extend_from_slice(&data[..remaining]);
        let block = std::mem::take(&mut self.chunk);
        self.compress(&block);

        let mut offset = remaining;
        while offset + 64 <= data.len() {
            self.compress(&data[offset..offset + 64]);
            offset += 64;
        }

        if offset < data.len() {
            self.chunk.extend_from_slice(&data[offset..]);
        }
    }

    fn fill(&mut self) {
        let bit_len = (self.size as u64) * 8;
        self.chunk.push(0x80);

        while self.chunk.len() % 64 != 56 {
            self.chunk.push(0);
        }

        self.chunk.extend_from_slice(&bit_len.to_be_bytes());
    }

    fn compress(&mut self, block: &[u8]) {
        debug_assert!(block.len() >= 64);

        let mut w = [0u32; 132];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[4 * i],
                block[4 * i + 1],
                block[4 * i + 2],
                block[4 * i + 3],
            ]);
        }

        for i in 16..68 {
            let a = w[i - 16] ^ w[i - 9] ^ rotl32(w[i - 3], 15);
            w[i] = p1(a) ^ rotl32(w[i - 13], 7) ^ w[i - 6];
        }

        for i in 0..64 {
            w[i + 68] = w[i] ^ w[i + 4];
        }

        let mut v = self.reg;
        for j in 0..64u32 {
            let j_idx = j as usize;
            let ss1 = rotl32(
                rotl32(v[0], 12)
                    .wrapping_add(v[4])
                    .wrapping_add(rotl32(tj(j_idx), j)),
                7,
            );
            let ss2 = ss1 ^ rotl32(v[0], 12);
            let tt1 = ff(j_idx, v[0], v[1], v[2])
                .wrapping_add(v[3])
                .wrapping_add(ss2)
                .wrapping_add(w[j_idx + 68]);
            let tt2 = gg(j_idx, v[4], v[5], v[6])
                .wrapping_add(v[7])
                .wrapping_add(ss1)
                .wrapping_add(w[j_idx]);

            v[3] = v[2];
            v[2] = rotl32(v[1], 9);
            v[1] = v[0];
            v[0] = tt1;
            v[7] = v[6];
            v[6] = rotl32(v[5], 19);
            v[5] = v[4];
            v[4] = p0(tt2);
        }

        for (i, vi) in v.iter().enumerate() {
            self.reg[i] ^= vi;
        }
    }

    fn sum(&mut self, input: Option<&[u8]>) -> [u8; 32] {
        if let Some(data) = input {
            self.reset();
            self.write_bytes(data);
        }

        self.fill();

        let chunk = std::mem::take(&mut self.chunk);
        for block in chunk.chunks(64) {
            self.compress(block);
        }

        let mut out = [0u8; 32];
        for (i, &word) in self.reg.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
        }

        self.reset();
        out
    }
}

#[inline]
fn rotl32(x: u32, n: u32) -> u32 {
    x.rotate_left(n)
}

#[inline]
fn tj(j: usize) -> u32 {
    if j < 16 { 0x79cc4519 } else { 0x7a879d8a }
}

#[inline]
fn ff(j: usize, x: u32, y: u32, z: u32) -> u32 {
    if j < 16 {
        x ^ y ^ z
    } else {
        (x & y) | (x & z) | (y & z)
    }
}

#[inline]
fn gg(j: usize, x: u32, y: u32, z: u32) -> u32 {
    if j < 16 {
        x ^ y ^ z
    } else {
        (x & y) | (!x & z)
    }
}

#[inline]
fn p0(x: u32) -> u32 {
    x ^ rotl32(x, 9) ^ rotl32(x, 17)
}

#[inline]
fn p1(x: u32) -> u32 {
    x ^ rotl32(x, 15) ^ rotl32(x, 23)
}

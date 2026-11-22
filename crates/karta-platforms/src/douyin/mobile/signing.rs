//! Mobile GET/POST request signing for Douyin's Android API.

use aes::Aes128;
use aes::cipher::{Block, BlockCipherEncrypt, KeyInit};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use super::super::a_bogus::sm3::sm3_hash;

const AID: &str = "1128";
const LICENSE: &str = "1611921764";

pub(super) fn sign_get(
    query: &str,
    device: &str,
    version: &str,
    time: u32,
) -> Vec<(&'static str, String)> {
    sign(query, device, version, time, None)
}

pub(super) fn sign(
    query: &str,
    device: &str,
    version: &str,
    time: u32,
    body: Option<&[u8]>,
) -> Vec<(&'static str, String)> {
    let body_hash = body.map(|bytes| md5::compute(bytes).0);
    let mut headers = vec![
        ("x-gorgon", gorgon_body(query, time, body_hash.as_ref())),
        (
            "x-argus",
            argus_body(
                query,
                device,
                version,
                time,
                rand::random::<u32>() & 0x7fff_ffff,
                body_hash.as_ref(),
            ),
        ),
        ("x-ladon", ladon(time, rand::random())),
        (
            "x-helios",
            helios(time, device, rand::random::<u32>() & 0x7fff_ffff),
        ),
        ("x-khronos", time.to_string()),
    ];
    if let Some(body) = body {
        headers.push(("x-ss-stub", format!("{:X}", md5::compute(body))));
        headers.push(("x-ss-req-ticket", (u64::from(time) * 1000).to_string()));
    }
    headers
}

fn padded(data: &[u8]) -> Vec<u8> {
    let pad = 16 - data.len() % 16;
    let mut out = data.to_vec();
    out.resize(data.len() + pad, pad as u8);
    out
}

#[cfg(test)]
fn gorgon(query: &str, time: u32) -> String {
    gorgon_body(query, time, None)
}

fn gorgon_body(query: &str, time: u32, body_hash: Option<&[u8; 16]>) -> String {
    let mut data = [0u8; 20];
    data[..4].copy_from_slice(&md5::compute(query).0[..4]);
    if let Some(hash) = body_hash {
        data[4..8].copy_from_slice(&hash[..4]);
    }
    data[16..].copy_from_slice(&time.to_be_bytes());
    let key = [0x1eu8, 0, 0xe0, 0xe4, 0x93, 0x45, 1, 0xd0];
    let mut table = std::array::from_fn::<_, 256, _>(|i| i as u8);
    let mut previous = None;
    for i in 0..256 {
        let mut a = if i == 0 {
            0
        } else {
            previous.filter(|&p| p != 0).unwrap_or(table[i - 1])
        };
        if a == 0x55 && i != 1 && previous != Some(0x55) {
            a = 0;
        }
        let c = a.wrapping_add(i as u8).wrapping_add(key[i % 8]);
        previous = if usize::from(c) < i { Some(c) } else { None };
        table[i] = table[usize::from(c)];
    }
    let mut mixed = table;
    let mut index = 0u8;
    for (i, byte) in data.iter_mut().enumerate() {
        index = table[i + 1].wrapping_add(index);
        let d = mixed[usize::from(index)];
        mixed[i + 1] = d;
        *byte ^= mixed[usize::from(d.wrapping_mul(2))];
    }
    for i in 0..data.len() {
        let b = data[i].rotate_left(4) ^ data[(i + 1) % data.len()];
        data[i] = !(b.reverse_bits() ^ 20);
    }
    let mut out = String::from("0404d0e40001");
    for byte in data {
        use std::fmt::Write;
        write!(out, "{byte:02x}").unwrap();
    }
    out
}

fn helios(time: u32, device: &str, nonce: u32) -> String {
    let mut material = nonce.to_le_bytes().to_vec();
    material.extend_from_slice(AID.as_bytes());
    let digest = format!("{:x}", md5::compute(&material));
    let cipher = Aes128::new_from_slice(&digest.as_bytes()[..16]).unwrap();
    let mut feedback = Block::<Aes128>::default();
    feedback.copy_from_slice(&digest.as_bytes()[16..]);
    let mut data = padded(format!("{time}-{device}-{AID}").as_bytes());
    for block in data.chunks_mut(16) {
        cipher.encrypt_block(&mut feedback);
        for (byte, mask) in block.iter_mut().zip(feedback.iter()) {
            *byte ^= mask;
        }
    }
    let mut packet = nonce.to_le_bytes().to_vec();
    packet.extend(data);
    STANDARD.encode(packet)
}

fn word(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().unwrap())
}

fn ladon(time: u32, nonce: [u8; 4]) -> String {
    let mut material = nonce.to_vec();
    material.extend_from_slice(AID.as_bytes());
    let digest = format!("{:x}", md5::compute(material));
    let mut keys = [0u64; 35];
    keys[0] = word(&digest.as_bytes()[..8]);
    let mut queue = std::collections::VecDeque::from([
        word(&digest.as_bytes()[8..16]),
        word(&digest.as_bytes()[16..24]),
        word(&digest.as_bytes()[24..]),
    ]);
    for i in 0..34 {
        let next = queue
            .pop_front()
            .unwrap()
            .rotate_right(8)
            .wrapping_add(keys[i])
            ^ i as u64;
        queue.push_back(next);
        keys[i + 1] = next ^ keys[i].rotate_left(3);
    }
    let mut data = padded(format!("{time}-{LICENSE}-{AID}").as_bytes());
    for block in data.chunks_mut(16) {
        let (mut a, mut b) = (word(&block[..8]), word(&block[8..]));
        for key in &keys[..34] {
            b = key ^ a.wrapping_add(b.rotate_right(8));
            a = b ^ a.rotate_left(3);
        }
        block[..8].copy_from_slice(&a.to_le_bytes());
        block[8..].copy_from_slice(&b.to_le_bytes());
    }
    let mut packet = nonce.to_vec();
    packet.extend(data);
    STANDARD.encode(packet)
}

fn varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        out.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}
fn number(out: &mut Vec<u8>, field: u64, value: u64) {
    varint(out, field << 3);
    varint(out, value);
}
fn bytes(out: &mut Vec<u8>, field: u64, value: &[u8]) {
    varint(out, (field << 3) | 2);
    varint(out, value.len() as u64);
    out.extend_from_slice(value);
}

fn simon(data: &mut [u8]) {
    // SM3-derived fixed key and Simon-128/256 schedule from the wire protocol.
    let material = [
        0xfc, 0x78, 0xe0, 0xa9, 0x65, 0x7a, 0x0c, 0x74, 0x8c, 0xe5, 0x15, 0x59, 0x90, 0x3c, 0xcf,
        0x03, 0x51, 0x0e, 0x51, 0xd3, 0xcf, 0xf2, 0x32, 0xd7, 0x13, 0x43, 0xe8, 0x8a, 0x32, 0x1c,
        0x53, 0x04,
    ];
    let mut keys = [0u64; 72];
    for (key, chunk) in keys.iter_mut().zip(material.as_chunks::<8>().0) {
        *key = word(chunk);
    }
    for i in 4..72 {
        let tmp = keys[i - 1].rotate_right(3) ^ keys[i - 3];
        keys[i] = !keys[i - 4]
            ^ tmp
            ^ tmp.rotate_right(1)
            ^ ((0x3dc94c3a046d678bu64 >> ((i - 4) % 62)) & 1)
            ^ 3;
    }
    for block in data.chunks_mut(16) {
        let (mut x, mut y) = (word(&block[..8]), word(&block[8..]));
        for key in keys {
            (x, y) = (
                y,
                x ^ (y.rotate_left(1) & y.rotate_left(8)) ^ y.rotate_left(2) ^ key,
            );
        }
        block[..8].copy_from_slice(&x.to_le_bytes());
        block[8..].copy_from_slice(&y.to_le_bytes());
    }
}

#[cfg(test)]
fn argus(query: &str, device: &str, version: &str, time: u32, nonce: u32) -> String {
    argus_body(query, device, version, time, nonce, None)
}

fn argus_body(
    query: &str,
    device: &str,
    version: &str,
    time: u32,
    nonce: u32,
    body_hash: Option<&[u8; 16]>,
) -> String {
    let mut pb = Vec::new();
    number(&mut pb, 1, 0x20200929 << 1);
    number(&mut pb, 2, 2);
    number(&mut pb, 3, u64::from(nonce));
    bytes(&mut pb, 4, AID.as_bytes());
    bytes(&mut pb, 5, device.as_bytes());
    bytes(&mut pb, 6, LICENSE.as_bytes());
    bytes(&mut pb, 7, version.as_bytes());
    bytes(&mut pb, 8, b"v04.04.05-ov-android");
    number(&mut pb, 9, 134744640);
    bytes(&mut pb, 10, &[0; 8]);
    number(&mut pb, 11, 0);
    number(&mut pb, 12, u64::from(time) << 1);
    bytes(&mut pb, 13, &sm3_hash(body_hash.unwrap_or(&[0; 16]))[..6]);
    bytes(&mut pb, 14, &sm3_hash(query.as_bytes())[..6]);
    let mut counters = Vec::new();
    for (field, value) in [(1, 1), (2, 1), (3, 1), (7, 3348294860)] {
        number(&mut counters, field, value);
    }
    bytes(&mut pb, 15, &counters);
    bytes(&mut pb, 16, b"");
    bytes(&mut pb, 20, b"none");
    number(&mut pb, 21, 738);
    let mut info = Vec::new();
    bytes(&mut info, 1, b"NX551J");
    number(&mut info, 2, 8196);
    number(&mut info, 4, 2162219008);
    bytes(&mut pb, 23, &info);
    number(&mut pb, 25, 2);
    let mut encrypted = padded(&pb);
    simon(&mut encrypted);
    let prefix = [0xf2, 0xf7, 0xfc, 0xff, 0xf2, 0xf7, 0xfc, 0xff];
    for (i, byte) in encrypted.iter_mut().enumerate() {
        *byte ^= prefix[i % 8];
    }
    let mut reversed = prefix.to_vec();
    reversed.extend(encrypted);
    reversed.reverse();
    let mut outer = vec![0xa6, 0x6e, 0xad, 0x9f, 0x77, 1, 0xd0, 0x0c, 0x18];
    outer.extend(reversed);
    outer.extend(b"ao");
    let sign_key = [
        0xac, 0x1a, 0xda, 0xae, 0x95, 0xa7, 0xaf, 0x94, 0xa5, 0x11, 0x4a, 0xb3, 0xb3, 0xa9, 0x7d,
        0xd8, 0, 0x50, 0xaa, 0x0a, 0x39, 0x31, 0x4c, 0x40, 0x52, 0x8c, 0xae, 0xc9, 0x52, 0x56,
        0xc2, 0x8c,
    ];
    let cipher = Aes128::new_from_slice(&md5::compute(&sign_key[..16]).0).unwrap();
    let mut previous = md5::compute(&sign_key[16..]).0;
    let mut packet = vec![0xf2, 0x81];
    for chunk in padded(&outer).as_chunks::<16>().0 {
        let mut block = Block::<Aes128>::default();
        for i in 0..16 {
            block[i] = chunk[i] ^ previous[i];
        }
        cipher.encrypt_block(&mut block);
        previous.copy_from_slice(&block);
        packet.extend_from_slice(&block);
    }
    STANDARD.encode(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn python_reference_get_vectors() {
        let q = "aweme_id=7688376340010315432&aid=1128&version_name=40.3.0&device_id=1234567890123456&ts=1700000000";
        assert_eq!(
            gorgon(q, 1700000000),
            "0404d0e4000186c509561704a5760372721ccf34c452cc984a74"
        );
        assert_eq!(
            ladon(1700000000, [13, 12, 11, 10]),
            "DQwLCkuxBkId2N3oF0iAGkTGNzK57fCOjvJpKDANdf4DHSF7"
        );
        assert_eq!(
            helios(1700000000, "1234567890123456", 0x0a0b0c0d),
            "DQwLCr9Qvrln5sDnVJRy53PYqbBlOoq1jdDfRmNy6bBo9wv8Xp8N4RIL8ZgeZjosOC+uCw=="
        );
        assert_eq!(
            argus(q, "1234567890123456", "40.3.0", 1700000000, 123456789),
            "8oGtly/q30zomF9jsMsfO3JH1WF7TLiPqtHvN1FuiHsrhc+PdIrYefXosEK6ZZjO/HCwRVrvBGVZq9XegJugsVjWf8QjPHQHf0KoNcgZ+p1Y1C4afQHmjGEIhsKEre3YRw0Y/0a4ztXICTsJ5x+Wkp3ZUsm87u1oqnJeK69FeHPm5gChoLB+lDI5q8fRQ9Usyw8kxFkE+pbM/qaJF/7XEVofSRSnY+8Xt9DGLLjFAEspM+N5fhc8tA8QDEWBixe8v/CX3LJukrcTL6AaKh5cVnp0"
        );
        assert_eq!(
            sign_get(q, "1234567890123456", "40.3.0", 1700000000)[4].1,
            "1700000000"
        );
    }
}

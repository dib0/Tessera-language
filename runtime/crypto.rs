// ---------------------------------------------------------------------------
// Tessera crypto runtime: the primitives TLS 1.3 needs and the `std/crypto`
// built-ins use. Pasted at top level after web.rs (it uses TwSha256,
// tw_chacha20_block, tw_words_le, tw_fill_random and tw_ct_eq from there).
// Safe Rust and std only; every item is prefixed `tw_` / `TW_` / `Tw`.
//
// Everything that touches secret data runs in constant time: no branches and
// no table lookups indexed by secret values. That is why AES is bitsliced
// (the S-box is computed as an inversion in GF(2^8), not looked up), GHASH
// multiplies bit by bit with masks, and the field and Montgomery arithmetic
// select with masks instead of branching. Public data (signatures, public
// keys, certificates) may take data-dependent time.
//
//   1. SHA-384 / SHA-512, HMAC and HKDF over SHA-256/384/512
//   2. ChaCha20-Poly1305 (RFC 8439)
//   3. AES-128/256 and GCM (FIPS 197, SP 800-38D)
//   4. X25519 (RFC 7748)
//   5. Montgomery arithmetic on fixed-length numbers
//   6. P-256 and P-384: ECDH and ECDSA (complete formulas, Renes-Costello-Batina 2015)
//   7. RSA: PKCS #1 v1.5 and PSS verification, PSS signing with CRT (RFC 8017)
//   8. Built-ins: sha512, hmac_sha512, hkdf_sha256, aead_seal, aead_open
// ---------------------------------------------------------------------------

// ===========================================================================
// 1. SHA-384 / SHA-512, HMAC, HKDF
// ===========================================================================

const TW_SHA512_K: [u64; 80] = [
    0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc, 0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118,
    0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2, 0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235, 0xc19bf174cf692694,
    0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65, 0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5,
    0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4, 0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70,
    0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df, 0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
    0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30, 0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8,
    0x19a4c116b8d2d0c8, 0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8, 0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec, 0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b,
    0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178, 0x06f067aa72176fba, 0x0a637dc5a2c898a6, 0x113f9804bef90dae, 0x1b710b35131c471b,
    0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c, 0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
];

#[derive(Clone)]
struct TwSha512 { h: [u64; 8], buf: [u8; 128], len: usize, total: u128, out_len: usize }

impl TwSha512 {
    fn new() -> TwSha512 {
        TwSha512 {
            h: [0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1, 0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179],
            buf: [0u8; 128], len: 0, total: 0, out_len: 64,
        }
    }
    fn new384() -> TwSha512 {
        TwSha512 {
            h: [0xcbbb9d5dc1059ed8, 0x629a292a367cd507, 0x9159015a3070dd17, 0x152fecd8f70e5939, 0x67332667ffc00b31, 0x8eb44a8768581511, 0xdb0c2e0d64f98fa7, 0x47b5481dbefa4fa4],
            buf: [0u8; 128], len: 0, total: 0, out_len: 48,
        }
    }
    fn compress(h: &mut [u64; 8], block: &[u8]) {
        let mut w = [0u64; 80];
        for t in 0..16 {
            let mut b = [0u8; 8];
            b.copy_from_slice(&block[t * 8..t * 8 + 8]);
            w[t] = u64::from_be_bytes(b);
        }
        for t in 16..80 {
            let s0 = w[t - 15].rotate_right(1) ^ w[t - 15].rotate_right(8) ^ (w[t - 15] >> 7);
            let s1 = w[t - 2].rotate_right(19) ^ w[t - 2].rotate_right(61) ^ (w[t - 2] >> 6);
            w[t] = w[t - 16].wrapping_add(s0).wrapping_add(w[t - 7]).wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) = (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for t in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(TW_SHA512_K[t]).wrapping_add(w[t]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g; g = f; f = e; e = d.wrapping_add(t1); d = c; c = b; b = a; a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a); h[1] = h[1].wrapping_add(b); h[2] = h[2].wrapping_add(c); h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e); h[5] = h[5].wrapping_add(f); h[6] = h[6].wrapping_add(g); h[7] = h[7].wrapping_add(hh);
    }
    fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u128);
        if self.len > 0 {
            let take = (128 - self.len).min(data.len());
            self.buf[self.len..self.len + take].copy_from_slice(&data[..take]);
            self.len += take;
            data = &data[take..];
            if self.len == 128 {
                TwSha512::compress(&mut self.h, &self.buf);
                self.len = 0;
            }
        }
        while data.len() >= 128 {
            TwSha512::compress(&mut self.h, &data[..128]);
            data = &data[128..];
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.len = data.len();
        }
    }
    fn finish(mut self) -> Vec<u8> {
        let bits = self.total.wrapping_mul(8);
        self.update(&[0x80]);
        while self.len != 112 { self.update(&[0]); }
        self.update(&bits.to_be_bytes());
        let mut out = Vec::with_capacity(64);
        for i in 0..8 { out.extend_from_slice(&self.h[i].to_be_bytes()); }
        out.truncate(self.out_len);
        out
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TwHashAlg { Sha256, Sha384, Sha512 }

impl TwHashAlg {
    fn len(self) -> usize { match self { TwHashAlg::Sha256 => 32, TwHashAlg::Sha384 => 48, TwHashAlg::Sha512 => 64 } }
    fn block(self) -> usize { match self { TwHashAlg::Sha256 => 64, _ => 128 } }
}

#[derive(Clone)]
enum TwHasher { S256(TwSha256), S512(TwSha512) }

impl TwHasher {
    fn new(alg: TwHashAlg) -> TwHasher {
        match alg {
            TwHashAlg::Sha256 => TwHasher::S256(TwSha256::new()),
            TwHashAlg::Sha384 => TwHasher::S512(TwSha512::new384()),
            TwHashAlg::Sha512 => TwHasher::S512(TwSha512::new()),
        }
    }
    fn update(&mut self, data: &[u8]) {
        match self { TwHasher::S256(h) => h.update(data), TwHasher::S512(h) => h.update(data) }
    }
    fn finish(self) -> Vec<u8> {
        match self { TwHasher::S256(h) => h.finish().to_vec(), TwHasher::S512(h) => h.finish() }
    }
}

fn tw_hash(alg: TwHashAlg, data: &[u8]) -> Vec<u8> {
    let mut h = TwHasher::new(alg);
    h.update(data);
    h.finish()
}

/// HMAC (RFC 2104) over any of the three hashes.
fn tw_hmac(alg: TwHashAlg, key: &[u8], message: &[u8]) -> Vec<u8> {
    let block = alg.block();
    let mut k = vec![0u8; block];
    if key.len() > block { let d = tw_hash(alg, key); k[..d.len()].copy_from_slice(&d); } else { k[..key.len()].copy_from_slice(key); }
    let mut inner = TwHasher::new(alg);
    inner.update(&k.iter().map(|b| b ^ 0x36).collect::<Vec<u8>>());
    inner.update(message);
    let digest = inner.finish();
    let mut outer = TwHasher::new(alg);
    outer.update(&k.iter().map(|b| b ^ 0x5c).collect::<Vec<u8>>());
    outer.update(&digest);
    outer.finish()
}

/// HKDF-Extract (RFC 5869). An empty salt acts as a string of zero bytes.
fn tw_hkdf_extract(alg: TwHashAlg, salt: &[u8], ikm: &[u8]) -> Vec<u8> { tw_hmac(alg, salt, ikm) }

/// HKDF-Expand (RFC 5869); `length` at most 255 hash lengths.
fn tw_hkdf_expand(alg: TwHashAlg, prk: &[u8], info: &[u8], length: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(length);
    let mut t: Vec<u8> = Vec::new();
    let mut counter = 1u8;
    while out.len() < length {
        let mut m = t.clone();
        m.extend_from_slice(info);
        m.push(counter);
        t = tw_hmac(alg, prk, &m);
        let take = t.len().min(length - out.len());
        out.extend_from_slice(&t[..take]);
        counter = counter.wrapping_add(1);
    }
    out
}

// ===========================================================================
// 2. ChaCha20-Poly1305 (RFC 8439)
// ===========================================================================

fn tw_le32(b: &[u8]) -> u32 { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) }

/// Poly1305 one-time authenticator, 26-bit limbs.
fn tw_poly1305(key: &[u8], msg: &[u8]) -> [u8; 16] {
    const M: u32 = 0x3ff_ffff;
    let r0 = tw_le32(&key[0..]) & 0x3ff_ffff;
    let r1 = (tw_le32(&key[3..]) >> 2) & 0x3ff_ff03;
    let r2 = (tw_le32(&key[6..]) >> 4) & 0x3ff_c0ff;
    let r3 = (tw_le32(&key[9..]) >> 6) & 0x3f0_3fff;
    let r4 = (tw_le32(&key[12..]) >> 8) & 0x00f_ffff;
    let (s1, s2, s3, s4) = (r1 * 5, r2 * 5, r3 * 5, r4 * 5);
    let (mut h0, mut h1, mut h2, mut h3, mut h4) = (0u32, 0u32, 0u32, 0u32, 0u32);
    for chunk in msg.chunks(16) {
        let mut b = [0u8; 17];
        b[..chunk.len()].copy_from_slice(chunk);
        let hibit: u32 = if chunk.len() == 16 { 1 << 24 } else { b[chunk.len()] = 1; 0 };
        h0 = h0.wrapping_add(tw_le32(&b[0..]) & M);
        h1 = h1.wrapping_add((tw_le32(&b[3..]) >> 2) & M);
        h2 = h2.wrapping_add((tw_le32(&b[6..]) >> 4) & M);
        h3 = h3.wrapping_add((tw_le32(&b[9..]) >> 6) & M);
        h4 = h4.wrapping_add((tw_le32(&b[12..]) >> 8) | hibit);
        let m = |a: u32, b: u32| a as u64 * b as u64;
        let d0 = m(h0, r0) + m(h1, s4) + m(h2, s3) + m(h3, s2) + m(h4, s1);
        let mut d1 = m(h0, r1) + m(h1, r0) + m(h2, s4) + m(h3, s3) + m(h4, s2);
        let mut d2 = m(h0, r2) + m(h1, r1) + m(h2, r0) + m(h3, s4) + m(h4, s3);
        let mut d3 = m(h0, r3) + m(h1, r2) + m(h2, r1) + m(h3, r0) + m(h4, s4);
        let mut d4 = m(h0, r4) + m(h1, r3) + m(h2, r2) + m(h3, r1) + m(h4, r0);
        let mut c = d0 >> 26; h0 = d0 as u32 & M;
        d1 += c; c = d1 >> 26; h1 = d1 as u32 & M;
        d2 += c; c = d2 >> 26; h2 = d2 as u32 & M;
        d3 += c; c = d3 >> 26; h3 = d3 as u32 & M;
        d4 += c; c = d4 >> 26; h4 = d4 as u32 & M;
        h0 = h0.wrapping_add((c as u32).wrapping_mul(5));
        let c2 = h0 >> 26; h0 &= M; h1 = h1.wrapping_add(c2);
    }
    let mut c = h1 >> 26; h1 &= M;
    h2 += c; c = h2 >> 26; h2 &= M;
    h3 += c; c = h3 >> 26; h3 &= M;
    h4 += c; c = h4 >> 26; h4 &= M;
    h0 += c * 5; c = h0 >> 26; h0 &= M;
    h1 += c;
    let mut g0 = h0.wrapping_add(5); c = g0 >> 26; g0 &= M;
    let mut g1 = h1.wrapping_add(c); c = g1 >> 26; g1 &= M;
    let mut g2 = h2.wrapping_add(c); c = g2 >> 26; g2 &= M;
    let mut g3 = h3.wrapping_add(c); c = g3 >> 26; g3 &= M;
    let mut g4 = h4.wrapping_add(c).wrapping_sub(1 << 26);
    let mask = (g4 >> 31).wrapping_sub(1); // all ones when h >= p: take g
    g0 &= mask; g1 &= mask; g2 &= mask; g3 &= mask; g4 &= mask;
    let keep = !mask;
    h0 = (h0 & keep) | g0; h1 = (h1 & keep) | g1; h2 = (h2 & keep) | g2; h3 = (h3 & keep) | g3; h4 = (h4 & keep) | g4;
    let w0 = h0 | (h1 << 26);
    let w1 = (h1 >> 6) | (h2 << 20);
    let w2 = (h2 >> 12) | (h3 << 14);
    let w3 = (h3 >> 18) | (h4 << 8);
    let mut f = w0 as u64 + tw_le32(&key[16..]) as u64;
    let o0 = f as u32;
    f = w1 as u64 + tw_le32(&key[20..]) as u64 + (f >> 32);
    let o1 = f as u32;
    f = w2 as u64 + tw_le32(&key[24..]) as u64 + (f >> 32);
    let o2 = f as u32;
    f = w3 as u64 + tw_le32(&key[28..]) as u64 + (f >> 32);
    let o3 = f as u32;
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&o0.to_le_bytes()); out[4..8].copy_from_slice(&o1.to_le_bytes());
    out[8..12].copy_from_slice(&o2.to_le_bytes()); out[12..16].copy_from_slice(&o3.to_le_bytes());
    out
}

fn tw_chacha_key(key: &[u8]) -> [u32; 8] {
    let k = tw_words_le(key);
    [k[0], k[1], k[2], k[3], k[4], k[5], k[6], k[7]]
}

fn tw_chacha20_xor(key: &[u32; 8], nonce: &[u32; 3], counter: u32, data: &mut [u8]) {
    let mut ctr = counter;
    for chunk in data.chunks_mut(64) {
        let block = tw_chacha20_block(key, ctr, nonce);
        for (d, k) in chunk.iter_mut().zip(block.iter()) { *d ^= k; }
        ctr = ctr.wrapping_add(1);
    }
}

fn tw_aead_mac_data(aad: &[u8], ct: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(aad.len() + ct.len() + 48);
    m.extend_from_slice(aad);
    while m.len() % 16 != 0 { m.push(0); }
    m.extend_from_slice(ct);
    while m.len() % 16 != 0 { m.push(0); }
    m.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    m.extend_from_slice(&(ct.len() as u64).to_le_bytes());
    m
}

/// ChaCha20-Poly1305 encryption: ciphertext followed by the 16-byte tag. Key 32 bytes, nonce 12.
fn tw_chacha20_poly1305_seal(key: &[u8], nonce: &[u8], aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let k = tw_chacha_key(key);
    let nw = tw_words_le(nonce);
    let n = [nw[0], nw[1], nw[2]];
    let otk = tw_chacha20_block(&k, 0, &n);
    let mut out = plaintext.to_vec();
    tw_chacha20_xor(&k, &n, 1, &mut out);
    let tag = tw_poly1305(&otk[..32], &tw_aead_mac_data(aad, &out));
    out.extend_from_slice(&tag);
    out
}

fn tw_chacha20_poly1305_open(key: &[u8], nonce: &[u8], aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() < 16 { return None; }
    let (ct, tag) = sealed.split_at(sealed.len() - 16);
    let k = tw_chacha_key(key);
    let nw = tw_words_le(nonce);
    let n = [nw[0], nw[1], nw[2]];
    let otk = tw_chacha20_block(&k, 0, &n);
    let expected = tw_poly1305(&otk[..32], &tw_aead_mac_data(aad, ct));
    if !tw_ct_eq(&expected, tag) { return None; }
    let mut out = ct.to_vec();
    tw_chacha20_xor(&k, &n, 1, &mut out);
    Some(out)
}

// ===========================================================================
// 3. AES and GCM
// ===========================================================================
//
// Bitsliced: up to four blocks (64 bytes) are transposed into eight 64-bit
// planes, plane b holding bit b of every byte. SubBytes is the inverse in
// GF(2^8) (x^254, by multiplying planes) followed by the affine map, so no
// secret-indexed table exists. ShiftRows and MixColumns work on bytes with
// fixed indexes and masks.

fn tw_gf8_mul_planes(a: &[u64; 8], b: &[u64; 8]) -> [u64; 8] {
    let mut c = [0u64; 15];
    for i in 0..8 { for j in 0..8 { c[i + j] ^= a[i] & b[j]; } }
    // x^8 = x^4 + x^3 + x + 1
    for k in (8..15).rev() {
        let t = c[k];
        c[k - 4] ^= t; c[k - 5] ^= t; c[k - 7] ^= t; c[k - 8] ^= t;
    }
    [c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]
}

fn tw_aes_sbox_planes(x: &[u64; 8]) -> [u64; 8] {
    let sq = |a: &[u64; 8]| tw_gf8_mul_planes(a, a);
    let x2 = sq(x);
    let x3 = tw_gf8_mul_planes(&x2, x);
    let x6 = sq(&x3);
    let x12 = sq(&x6);
    let x15 = tw_gf8_mul_planes(&x12, &x3);
    let x240 = sq(&sq(&sq(&sq(&x15))));
    let x252 = tw_gf8_mul_planes(&x240, &x12);
    let inv = tw_gf8_mul_planes(&x252, &x2); // x^254: the inverse, 0 for 0
    let mut s = [0u64; 8];
    for i in 0..8 {
        s[i] = inv[i] ^ inv[(i + 4) % 8] ^ inv[(i + 5) % 8] ^ inv[(i + 6) % 8] ^ inv[(i + 7) % 8];
    }
    // constant 0x63: bits 0, 1, 5, 6
    s[0] = !s[0]; s[1] = !s[1]; s[5] = !s[5]; s[6] = !s[6];
    s
}

/// SubBytes on up to 64 bytes at once.
fn tw_aes_sub_bytes(state: &mut [u8]) {
    let mut p = [0u64; 8];
    for (i, &v) in state.iter().enumerate() {
        for b in 0..8 { p[b] |= (((v >> b) & 1) as u64) << i; }
    }
    let s = tw_aes_sbox_planes(&p);
    for (i, v) in state.iter_mut().enumerate() {
        let mut o = 0u8;
        for b in 0..8 { o |= (((s[b] >> i) & 1) as u8) << b; }
        *v = o;
    }
}

fn tw_xtime(x: u8) -> u8 { (x << 1) ^ (0x1b & 0u8.wrapping_sub(x >> 7)) }

#[derive(Clone)]
struct TwAes { rk: Vec<[u8; 16]> }

impl TwAes {
    /// Key of 16 bytes (AES-128) or 32 bytes (AES-256).
    fn new(key: &[u8]) -> TwAes {
        let nk = key.len() / 4;
        let nr = nk + 6;
        let mut w: Vec<[u8; 4]> = Vec::with_capacity(4 * (nr + 1));
        for i in 0..nk { w.push([key[4 * i], key[4 * i + 1], key[4 * i + 2], key[4 * i + 3]]); }
        let mut rcon = 1u8;
        for i in nk..4 * (nr + 1) {
            let mut t = w[i - 1];
            if i % nk == 0 {
                t = [t[1], t[2], t[3], t[0]];
                tw_aes_sub_bytes(&mut t);
                t[0] ^= rcon;
                rcon = tw_xtime(rcon);
            } else if nk > 6 && i % nk == 4 {
                tw_aes_sub_bytes(&mut t);
            }
            let p = w[i - nk];
            w.push([p[0] ^ t[0], p[1] ^ t[1], p[2] ^ t[2], p[3] ^ t[3]]);
        }
        let mut rk = Vec::with_capacity(nr + 1);
        for r in 0..=nr {
            let mut k = [0u8; 16];
            for c in 0..4 { k[4 * c..4 * c + 4].copy_from_slice(&w[4 * r + c]); }
            rk.push(k);
        }
        TwAes { rk }
    }
    /// Encrypts up to four blocks in place.
    fn encrypt_blocks(&self, blocks: &mut [[u8; 16]]) {
        let n = blocks.len();
        let nr = self.rk.len() - 1;
        let mut flat = [0u8; 64];
        for (i, b) in blocks.iter().enumerate() { flat[16 * i..16 * i + 16].copy_from_slice(b); }
        for i in 0..n { for j in 0..16 { flat[16 * i + j] ^= self.rk[0][j]; } }
        for round in 1..=nr {
            tw_aes_sub_bytes(&mut flat[..16 * n]);
            for i in 0..n {
                let s = &mut flat[16 * i..16 * i + 16];
                let old = [s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7], s[8], s[9], s[10], s[11], s[12], s[13], s[14], s[15]];
                for c in 0..4 { for r in 0..4 { s[r + 4 * c] = old[r + 4 * ((c + r) % 4)]; } }
                if round != nr {
                    for c in 0..4 {
                        let a = [s[4 * c], s[4 * c + 1], s[4 * c + 2], s[4 * c + 3]];
                        let all = a[0] ^ a[1] ^ a[2] ^ a[3];
                        for r in 0..4 { s[4 * c + r] = a[r] ^ all ^ tw_xtime(a[r] ^ a[(r + 1) % 4]); }
                    }
                }
                for j in 0..16 { s[j] ^= self.rk[round][j]; }
            }
        }
        for (i, b) in blocks.iter_mut().enumerate() { b.copy_from_slice(&flat[16 * i..16 * i + 16]); }
    }
}

/// GF(2^128) multiplication in GCM's bit order, bit by bit with masks.
fn tw_gf128_mul(x: u128, y: u128) -> u128 {
    let r: u128 = 0xe1 << 120;
    let mut z = 0u128;
    let mut v = y;
    for i in 0..128 {
        let bit = (x >> (127 - i)) & 1;
        z ^= v & 0u128.wrapping_sub(bit);
        let lsb = v & 1;
        v = (v >> 1) ^ (r & 0u128.wrapping_sub(lsb));
    }
    z
}

#[derive(Clone)]
struct TwAesGcm { aes: TwAes, h: u128 }

impl TwAesGcm {
    fn new(key: &[u8]) -> TwAesGcm {
        let aes = TwAes::new(key);
        let mut z = [[0u8; 16]];
        aes.encrypt_blocks(&mut z);
        TwAesGcm { aes, h: u128::from_be_bytes(z[0]) }
    }
    fn ctr(&self, nonce: &[u8], data: &mut [u8]) {
        let mut counter = 2u32;
        for group in data.chunks_mut(64) {
            let nblocks = (group.len() + 15) / 16;
            let mut ks = [[0u8; 16]; 4];
            for b in ks.iter_mut().take(nblocks) {
                b[..12].copy_from_slice(nonce);
                b[12..].copy_from_slice(&counter.to_be_bytes());
                counter = counter.wrapping_add(1);
            }
            self.aes.encrypt_blocks(&mut ks[..nblocks]);
            for (i, d) in group.iter_mut().enumerate() { *d ^= ks[i / 16][i % 16]; }
        }
    }
    fn tag(&self, nonce: &[u8], aad: &[u8], ct: &[u8]) -> [u8; 16] {
        let mut y = 0u128;
        for part in [aad, ct] {
            for chunk in part.chunks(16) {
                let mut b = [0u8; 16];
                b[..chunk.len()].copy_from_slice(chunk);
                y = tw_gf128_mul(y ^ u128::from_be_bytes(b), self.h);
            }
        }
        let lens = ((aad.len() as u128 * 8) << 64) | (ct.len() as u128 * 8);
        y = tw_gf128_mul(y ^ lens, self.h);
        let mut j0 = [[0u8; 16]];
        j0[0][..12].copy_from_slice(nonce);
        j0[0][15] = 1;
        self.aes.encrypt_blocks(&mut j0);
        (y ^ u128::from_be_bytes(j0[0])).to_be_bytes()
    }
    fn seal(&self, nonce: &[u8], aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
        let mut out = plaintext.to_vec();
        self.ctr(nonce, &mut out);
        let t = self.tag(nonce, aad, &out);
        out.extend_from_slice(&t);
        out
    }
    fn open(&self, nonce: &[u8], aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
        if sealed.len() < 16 { return None; }
        let (ct, tag) = sealed.split_at(sealed.len() - 16);
        if !tw_ct_eq(&self.tag(nonce, aad, ct), tag) { return None; }
        let mut out = ct.to_vec();
        self.ctr(nonce, &mut out);
        Some(out)
    }
}

// ===========================================================================
// 4. X25519 (RFC 7748): field elements as five 51-bit limbs
// ===========================================================================

type TwFe = [u64; 5];
const TW_MASK51: u64 = (1 << 51) - 1;

fn tw_le64(b: &[u8]) -> u64 { let mut a = [0u8; 8]; a.copy_from_slice(&b[..8]); u64::from_le_bytes(a) }

fn tw_fe_from_bytes(b: &[u8]) -> TwFe {
    [
        tw_le64(&b[0..]) & TW_MASK51,
        (tw_le64(&b[6..]) >> 3) & TW_MASK51,
        (tw_le64(&b[12..]) >> 6) & TW_MASK51,
        (tw_le64(&b[19..]) >> 1) & TW_MASK51,
        (tw_le64(&b[24..]) >> 12) & TW_MASK51,
    ]
}
fn tw_fe_carry(mut h: TwFe) -> TwFe {
    let mut c;
    c = h[0] >> 51; h[0] &= TW_MASK51; h[1] += c;
    c = h[1] >> 51; h[1] &= TW_MASK51; h[2] += c;
    c = h[2] >> 51; h[2] &= TW_MASK51; h[3] += c;
    c = h[3] >> 51; h[3] &= TW_MASK51; h[4] += c;
    c = h[4] >> 51; h[4] &= TW_MASK51; h[0] += c * 19;
    c = h[0] >> 51; h[0] &= TW_MASK51; h[1] += c;
    h
}
fn tw_fe_add(a: &TwFe, b: &TwFe) -> TwFe { tw_fe_carry([a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3], a[4] + b[4]]) }
fn tw_fe_sub(a: &TwFe, b: &TwFe) -> TwFe {
    // a + 4p - b keeps every limb positive for carried inputs
    tw_fe_carry([
        a[0] + 0x1f_ffff_ffff_ffb4 - b[0],
        a[1] + 0x1f_ffff_ffff_fffc - b[1],
        a[2] + 0x1f_ffff_ffff_fffc - b[2],
        a[3] + 0x1f_ffff_ffff_fffc - b[3],
        a[4] + 0x1f_ffff_ffff_fffc - b[4],
    ])
}
fn tw_fe_mul(a: &TwFe, b: &TwFe) -> TwFe {
    let m = |x: u64, y: u64| x as u128 * y as u128;
    let (b1, b2, b3, b4) = (b[1] * 19, b[2] * 19, b[3] * 19, b[4] * 19);
    let r0 = m(a[0], b[0]) + m(a[1], b4) + m(a[2], b3) + m(a[3], b2) + m(a[4], b1);
    let mut r1 = m(a[0], b[1]) + m(a[1], b[0]) + m(a[2], b4) + m(a[3], b3) + m(a[4], b2);
    let mut r2 = m(a[0], b[2]) + m(a[1], b[1]) + m(a[2], b[0]) + m(a[3], b4) + m(a[4], b3);
    let mut r3 = m(a[0], b[3]) + m(a[1], b[2]) + m(a[2], b[1]) + m(a[3], b[0]) + m(a[4], b4);
    let mut r4 = m(a[0], b[4]) + m(a[1], b[3]) + m(a[2], b[2]) + m(a[3], b[1]) + m(a[4], b[0]);
    let mask = TW_MASK51 as u128;
    let mut c = r0 >> 51; let h0 = (r0 & mask) as u64;
    r1 += c; c = r1 >> 51; let h1 = (r1 & mask) as u64;
    r2 += c; c = r2 >> 51; let h2 = (r2 & mask) as u64;
    r3 += c; c = r3 >> 51; let h3 = (r3 & mask) as u64;
    r4 += c; c = r4 >> 51; let h4 = (r4 & mask) as u64;
    let h0 = h0 + (c as u64) * 19;
    tw_fe_carry([h0, h1, h2, h3, h4])
}
fn tw_fe_sq(a: &TwFe) -> TwFe { tw_fe_mul(a, a) }
/// z^(p-2) with p = 2^255 - 19; the exponent is public.
fn tw_fe_invert(z: &TwFe) -> TwFe {
    // p - 2 = 2^255 - 21: bits 254..5 set, then 01011
    let mut r: TwFe = [1, 0, 0, 0, 0];
    for i in (0..255).rev() {
        r = tw_fe_sq(&r);
        let bit = if i >= 5 { 1 } else { (0b01011u32 >> i) & 1 };
        if bit == 1 { r = tw_fe_mul(&r, z); }
    }
    r
}
fn tw_fe_to_bytes(h: &TwFe) -> [u8; 32] {
    let mut h = tw_fe_carry(tw_fe_carry(*h));
    let mut q = (h[0] + 19) >> 51;
    q = (h[1] + q) >> 51; q = (h[2] + q) >> 51; q = (h[3] + q) >> 51; q = (h[4] + q) >> 51;
    h[0] += 19 * q;
    let mut c;
    c = h[0] >> 51; h[0] &= TW_MASK51; h[1] += c;
    c = h[1] >> 51; h[1] &= TW_MASK51; h[2] += c;
    c = h[2] >> 51; h[2] &= TW_MASK51; h[3] += c;
    c = h[3] >> 51; h[3] &= TW_MASK51; h[4] += c;
    h[4] &= TW_MASK51;
    let w = [h[0] | (h[1] << 51), (h[1] >> 13) | (h[2] << 38), (h[2] >> 26) | (h[3] << 25), (h[3] >> 39) | (h[4] << 12)];
    let mut out = [0u8; 32];
    for i in 0..4 { out[8 * i..8 * i + 8].copy_from_slice(&w[i].to_le_bytes()); }
    out
}
fn tw_fe_cswap(swap: u64, a: &mut TwFe, b: &mut TwFe) {
    let mask = 0u64.wrapping_sub(swap);
    for i in 0..5 { let t = mask & (a[i] ^ b[i]); a[i] ^= t; b[i] ^= t; }
}

/// X25519(scalar, u): both 32 bytes.
fn tw_x25519(scalar: &[u8], u: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 32];
    k.copy_from_slice(&scalar[..32]);
    k[0] &= 248; k[31] &= 127; k[31] |= 64;
    let x1 = tw_fe_from_bytes(u);
    let mut x2: TwFe = [1, 0, 0, 0, 0];
    let mut z2: TwFe = [0; 5];
    let mut x3 = x1;
    let mut z3: TwFe = [1, 0, 0, 0, 0];
    let a24: TwFe = [121665, 0, 0, 0, 0];
    let mut swap = 0u64;
    for t in (0..255).rev() {
        let kt = ((k[t / 8] >> (t % 8)) & 1) as u64;
        swap ^= kt;
        tw_fe_cswap(swap, &mut x2, &mut x3);
        tw_fe_cswap(swap, &mut z2, &mut z3);
        swap = kt;
        let a = tw_fe_add(&x2, &z2);
        let aa = tw_fe_sq(&a);
        let b = tw_fe_sub(&x2, &z2);
        let bb = tw_fe_sq(&b);
        let e = tw_fe_sub(&aa, &bb);
        let c = tw_fe_add(&x3, &z3);
        let d = tw_fe_sub(&x3, &z3);
        let da = tw_fe_mul(&d, &a);
        let cb = tw_fe_mul(&c, &b);
        x3 = tw_fe_sq(&tw_fe_add(&da, &cb));
        z3 = tw_fe_mul(&x1, &tw_fe_sq(&tw_fe_sub(&da, &cb)));
        x2 = tw_fe_mul(&aa, &bb);
        z2 = tw_fe_mul(&e, &tw_fe_add(&aa, &tw_fe_mul(&a24, &e)));
    }
    tw_fe_cswap(swap, &mut x2, &mut x3);
    tw_fe_cswap(swap, &mut z2, &mut z3);
    tw_fe_to_bytes(&tw_fe_mul(&x2, &tw_fe_invert(&z2)))
}

fn tw_x25519_base(scalar: &[u8]) -> [u8; 32] {
    let mut nine = [0u8; 32];
    nine[0] = 9;
    tw_x25519(scalar, &nine)
}

// ===========================================================================
// 5. Montgomery arithmetic on fixed-length numbers (little-endian u64 limbs)
// ===========================================================================

/// Big-endian bytes to `limbs` limbs; None when the value does not fit.
fn tw_bn_from_be(bytes: &[u8], limbs: usize) -> Option<Vec<u64>> {
    let mut start = 0;
    while start < bytes.len() && bytes[start] == 0 && bytes.len() - start > limbs * 8 { start += 1; }
    let b = &bytes[start..];
    if b.len() > limbs * 8 { return None; }
    let mut out = vec![0u64; limbs];
    for (i, &byte) in b.iter().rev().enumerate() { out[i / 8] |= (byte as u64) << (8 * (i % 8)); }
    Some(out)
}
fn tw_bn_to_be(a: &[u64], len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    for i in 0..len {
        let limb = i / 8;
        if limb < a.len() { out[len - 1 - i] = (a[limb] >> (8 * (i % 8))) as u8; }
    }
    out
}
/// a - b and the final borrow (0 or 1).
fn tw_bn_sub(a: &[u64], b: &[u64]) -> (Vec<u64>, u64) {
    let mut out = vec![0u64; a.len()];
    let mut borrow = 0u64;
    for i in 0..a.len() {
        let bi = if i < b.len() { b[i] } else { 0 };
        let (d1, o1) = a[i].overflowing_sub(bi);
        let (d2, o2) = d1.overflowing_sub(borrow);
        out[i] = d2;
        borrow = (o1 | o2) as u64;
    }
    (out, borrow)
}
fn tw_bn_add(a: &[u64], b: &[u64]) -> (Vec<u64>, u64) {
    let mut out = vec![0u64; a.len()];
    let mut carry = 0u64;
    for i in 0..a.len() {
        let bi = if i < b.len() { b[i] } else { 0 };
        let s = a[i] as u128 + bi as u128 + carry as u128;
        out[i] = s as u64;
        carry = (s >> 64) as u64;
    }
    (out, carry)
}
/// Constant-time: mask all ones selects `a`, zero selects `b`.
fn tw_bn_select(mask: u64, a: &[u64], b: &[u64]) -> Vec<u64> {
    a.iter().zip(b.iter()).map(|(x, y)| (x & mask) | (y & !mask)).collect()
}
fn tw_bn_is_zero(a: &[u64]) -> bool { a.iter().fold(0u64, |acc, x| acc | x) == 0 }
fn tw_bn_lt(a: &[u64], b: &[u64]) -> bool { tw_bn_sub(a, b).1 == 1 }
fn tw_bn_bits(a: &[u64]) -> usize {
    for i in (0..a.len()).rev() { if a[i] != 0 { return 64 * i + 64 - a[i].leading_zeros() as usize; } }
    0
}
/// Plain product, a.len() + b.len() limbs.
fn tw_bn_mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = vec![0u64; a.len() + b.len()];
    for i in 0..a.len() {
        let mut carry = 0u128;
        for j in 0..b.len() {
            let t = out[i + j] as u128 + a[i] as u128 * b[j] as u128 + carry;
            out[i + j] = t as u64;
            carry = t >> 64;
        }
        out[i + b.len()] = carry as u64;
    }
    out
}

#[derive(Clone)]
struct TwMont { m: Vec<u64>, n0: u64, r2: Vec<u64>, one: Vec<u64> }

impl TwMont {
    /// `m` must be odd and its top limb non-zero.
    fn new(m: Vec<u64>) -> TwMont {
        let mut inv = 1u64;
        for _ in 0..6 { inv = inv.wrapping_mul(2u64.wrapping_sub(m[0].wrapping_mul(inv))); }
        let n0 = inv.wrapping_neg();
        let n = m.len();
        let mut mont = TwMont { m, n0, r2: Vec::new(), one: Vec::new() };
        // R^2 mod m by doubling 1 (2 * 64 * n) times
        let mut x = vec![0u64; n];
        x[0] = 1;
        let x = { let mut v = mont.reduce_once(x); for _ in 0..128 * n { v = mont.add(&v, &v); } v };
        mont.r2 = x;
        let mut unit = vec![0u64; n];
        unit[0] = 1;
        mont.one = mont.mul(&unit, &mont.r2.clone());
        mont
    }
    fn len(&self) -> usize { self.m.len() }
    fn reduce_once(&self, a: Vec<u64>) -> Vec<u64> {
        let (d, borrow) = tw_bn_sub(&a, &self.m);
        tw_bn_select(0u64.wrapping_sub(borrow), &a, &d)
    }
    /// a * b / R mod m (CIOS). Requires a < R and b < m.
    fn mul(&self, a: &[u64], b: &[u64]) -> Vec<u64> {
        let n = self.m.len();
        let mut t = vec![0u64; n + 2];
        for i in 0..n {
            let mut c = 0u128;
            for j in 0..n {
                let v = t[j] as u128 + a[j] as u128 * b[i] as u128 + c;
                t[j] = v as u64;
                c = v >> 64;
            }
            let v = t[n] as u128 + c;
            t[n] = v as u64;
            t[n + 1] = (v >> 64) as u64;
            let mm = t[0].wrapping_mul(self.n0);
            let v = t[0] as u128 + mm as u128 * self.m[0] as u128;
            let mut c = v >> 64;
            for j in 1..n {
                let v = t[j] as u128 + mm as u128 * self.m[j] as u128 + c;
                t[j - 1] = v as u64;
                c = v >> 64;
            }
            let v = t[n] as u128 + c;
            t[n - 1] = v as u64;
            t[n] = t[n + 1] + (v >> 64) as u64;
        }
        let top = t[n];
        t.truncate(n);
        let (d, borrow) = tw_bn_sub(&t, &self.m);
        // keep t only when it is below m: no carry out and a borrow
        let keep_t = 0u64.wrapping_sub(borrow & (1 ^ (top & 1)));
        tw_bn_select(keep_t, &t, &d)
    }
    fn add(&self, a: &[u64], b: &[u64]) -> Vec<u64> {
        let (s, carry) = tw_bn_add(a, b);
        let (d, borrow) = tw_bn_sub(&s, &self.m);
        let keep_s = 0u64.wrapping_sub(borrow & (1 ^ carry));
        tw_bn_select(keep_s, &s, &d)
    }
    fn sub(&self, a: &[u64], b: &[u64]) -> Vec<u64> {
        let (d, borrow) = tw_bn_sub(a, b);
        let mask = 0u64.wrapping_sub(borrow);
        let masked: Vec<u64> = self.m.iter().map(|x| x & mask).collect();
        tw_bn_add(&d, &masked).0
    }
    fn to_mont(&self, a: &[u64]) -> Vec<u64> { self.mul(a, &self.r2) }
    fn from_mont(&self, a: &[u64]) -> Vec<u64> {
        let mut unit = vec![0u64; self.len()];
        unit[0] = 1;
        self.mul(a, &unit)
    }
    /// Any big-endian number of at most 2 * len limbs, reduced mod m (normal form).
    fn reduce_bytes(&self, bytes: &[u8]) -> Option<Vec<u64>> {
        let n = self.len();
        let wide = tw_bn_from_be(bytes, 2 * n)?;
        let lo = &wide[..n];
        let hi = &wide[n..];
        let a = self.mul(lo, &self.r2); // lo * R
        let b = self.mul(&self.mul(hi, &self.r2), &self.r2); // hi * R^2
        Some(self.from_mont(&self.add(&a, &b)))
    }
    /// base^exp with `base` in Montgomery form; exp big-endian. Fixed 4-bit
    /// windows and a table read that touches every entry: constant time in the
    /// exponent's value (its length is public).
    fn pow(&self, base: &[u64], exp: &[u8]) -> Vec<u64> {
        let mut table: Vec<Vec<u64>> = Vec::with_capacity(16);
        table.push(self.one.clone());
        for i in 1..16 { let next = self.mul(&table[i - 1], base); table.push(next); }
        let mut r = self.one.clone();
        for &byte in exp {
            for nibble in [byte >> 4, byte & 15] {
                for _ in 0..4 { r = self.mul(&r, &r); }
                let mut sel = vec![0u64; self.len()];
                for (i, t) in table.iter().enumerate() {
                    let eq = ((i as u64 ^ nibble as u64).wrapping_sub(1) >> 63) & 1; // 1 when equal
                    let mask = 0u64.wrapping_sub(eq);
                    for k in 0..sel.len() { sel[k] |= t[k] & mask; }
                }
                r = self.mul(&r, &sel);
            }
        }
        r
    }
    /// Inverse for a prime modulus (Fermat), Montgomery form in and out.
    fn invert_prime(&self, a: &[u64]) -> Vec<u64> {
        let mut two = vec![0u64; self.len()];
        two[0] = 2;
        let e = tw_bn_sub(&self.m, &two).0;
        self.pow(a, &tw_bn_to_be(&e, self.len() * 8))
    }
}

// ===========================================================================
// 6. P-256 and P-384 (short Weierstrass, a = -3)
// ===========================================================================

struct TwCurve { name: &'static str, fp: TwMont, fnn: TwMont, b: Vec<u64>, gx: Vec<u64>, gy: Vec<u64>, size: usize }

fn tw_hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap_or(0)).collect()
}

fn tw_make_curve(name: &'static str, p: &str, n: &str, b: &str, gx: &str, gy: &str) -> TwCurve {
    let size = p.len() / 2;
    let limbs = (size + 7) / 8;
    let fp = TwMont::new(tw_bn_from_be(&tw_hex_bytes(p), limbs).unwrap_or_default());
    let fnn = TwMont::new(tw_bn_from_be(&tw_hex_bytes(n), limbs).unwrap_or_default());
    let to = |h: &str| fp.to_mont(&tw_bn_from_be(&tw_hex_bytes(h), limbs).unwrap_or_default());
    let (b, gx, gy) = (to(b), to(gx), to(gy));
    TwCurve { name, fp, fnn, b, gx, gy, size }
}

static TW_P256: std::sync::OnceLock<TwCurve> = std::sync::OnceLock::new();
static TW_P384: std::sync::OnceLock<TwCurve> = std::sync::OnceLock::new();

fn tw_p256() -> &'static TwCurve {
    TW_P256.get_or_init(|| tw_make_curve("P-256",
        "ffffffff00000001000000000000000000000000ffffffffffffffffffffffff",
        "ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551",
        "5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b",
        "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
        "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"))
}
fn tw_p384() -> &'static TwCurve {
    TW_P384.get_or_init(|| tw_make_curve("P-384",
        "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeffffffff0000000000000000ffffffff",
        "ffffffffffffffffffffffffffffffffffffffffffffffffc7634d81f4372ddf581a0db248b0a77aecec196accc52973",
        "b3312fa7e23ee7e4988e056be3f82d19181d9c6efe8141120314088f5013875ac656398d8a2ed19d2a85c8edd3ec2aef",
        "aa87ca22be8b05378eb1c71ef320ad746e1d3b628ba79b9859f741e082542a385502f25dbf55296c3a545e3872760ab7",
        "3617de4a96262c6f5d9e98bf9292dc29f8f41dbd289a147ce9da3113b5f0b8c00a60b1ce1d7e819d7a431d7c90ea0e5f"))
}

/// Projective point (X : Y : Z), coordinates in Montgomery form; Z = 0 is the point at infinity.
#[derive(Clone)]
struct TwPoint { x: Vec<u64>, y: Vec<u64>, z: Vec<u64> }

impl TwCurve {
    fn infinity(&self) -> TwPoint { TwPoint { x: vec![0; self.fp.len()], y: self.fp.one.clone(), z: vec![0; self.fp.len()] } }
    fn generator(&self) -> TwPoint { TwPoint { x: self.gx.clone(), y: self.gy.clone(), z: self.fp.one.clone() } }
    /// Complete addition for a = -3 (Renes, Costello, Batina 2015, algorithm 4);
    /// also correct for doubling and the point at infinity.
    fn add(&self, p: &TwPoint, q: &TwPoint) -> TwPoint {
        let f = &self.fp;
        let mul = |a: &[u64], b: &[u64]| f.mul(a, b);
        let add = |a: &[u64], b: &[u64]| f.add(a, b);
        let sub = |a: &[u64], b: &[u64]| f.sub(a, b);
        let (x1, y1, z1, x2, y2, z2) = (&p.x, &p.y, &p.z, &q.x, &q.y, &q.z);
        let mut t0 = mul(x1, x2);
        let mut t1 = mul(y1, y2);
        let mut t2 = mul(z1, z2);
        let mut t3 = add(x1, y1);
        let mut t4 = add(x2, y2);
        t3 = mul(&t3, &t4);
        t4 = add(&t0, &t1);
        t3 = sub(&t3, &t4);
        t4 = add(y1, z1);
        let mut x3 = add(y2, z2);
        t4 = mul(&t4, &x3);
        x3 = add(&t1, &t2);
        t4 = sub(&t4, &x3);
        x3 = add(x1, z1);
        let mut y3 = add(x2, z2);
        x3 = mul(&x3, &y3);
        y3 = add(&t0, &t2);
        y3 = sub(&x3, &y3);
        let mut z3 = mul(&self.b, &t2);
        x3 = sub(&y3, &z3);
        z3 = add(&x3, &x3);
        x3 = add(&x3, &z3);
        z3 = sub(&t1, &x3);
        x3 = add(&t1, &x3);
        y3 = mul(&self.b, &y3);
        t1 = add(&t2, &t2);
        t2 = add(&t1, &t2);
        y3 = sub(&y3, &t2);
        y3 = sub(&y3, &t0);
        t1 = add(&y3, &y3);
        y3 = add(&t1, &y3);
        t1 = add(&t0, &t0);
        t0 = add(&t1, &t0);
        t0 = sub(&t0, &t2);
        t1 = mul(&t4, &y3);
        t2 = mul(&t0, &y3);
        y3 = mul(&x3, &z3);
        y3 = add(&y3, &t2);
        x3 = mul(&t3, &x3);
        x3 = sub(&x3, &t1);
        z3 = mul(&t4, &z3);
        t1 = mul(&t3, &t0);
        z3 = add(&z3, &t1);
        TwPoint { x: x3, y: y3, z: z3 }
    }
    /// k * P for a big-endian scalar; double and always add, selection by mask.
    fn mul_scalar(&self, k: &[u8], p: &TwPoint) -> TwPoint {
        let mut r = self.infinity();
        for &byte in k {
            for bit in (0..8).rev() {
                r = self.add(&r, &r);
                let t = self.add(&r, p);
                let mask = 0u64.wrapping_sub(((byte >> bit) & 1) as u64);
                r = TwPoint { x: tw_bn_select(mask, &t.x, &r.x), y: tw_bn_select(mask, &t.y, &r.y), z: tw_bn_select(mask, &t.z, &r.z) };
            }
        }
        r
    }
    /// Affine (x, y) in normal form; None for the point at infinity.
    fn to_affine(&self, p: &TwPoint) -> Option<(Vec<u64>, Vec<u64>)> {
        if tw_bn_is_zero(&p.z) { return None; }
        let zi = self.fp.invert_prime(&p.z);
        Some((self.fp.from_mont(&self.fp.mul(&p.x, &zi)), self.fp.from_mont(&self.fp.mul(&p.y, &zi))))
    }
    fn encode_point(&self, p: &TwPoint) -> Option<Vec<u8>> {
        let (x, y) = self.to_affine(p)?;
        let mut out = vec![4u8];
        out.extend_from_slice(&tw_bn_to_be(&x, self.size));
        out.extend_from_slice(&tw_bn_to_be(&y, self.size));
        Some(out)
    }
    /// Uncompressed point 04 || x || y, checked to be on the curve.
    fn decode_point(&self, data: &[u8]) -> Option<TwPoint> {
        if data.len() != 1 + 2 * self.size || data[0] != 4 { return None; }
        let n = self.fp.len();
        let x = tw_bn_from_be(&data[1..1 + self.size], n)?;
        let y = tw_bn_from_be(&data[1 + self.size..], n)?;
        if !tw_bn_lt(&x, &self.fp.m) || !tw_bn_lt(&y, &self.fp.m) { return None; }
        let (xm, ym) = (self.fp.to_mont(&x), self.fp.to_mont(&y));
        let f = &self.fp;
        let y2 = f.mul(&ym, &ym);
        let x3 = f.mul(&f.mul(&xm, &xm), &xm);
        let three_x = f.add(&f.add(&xm, &xm), &xm);
        let rhs = f.add(&f.sub(&x3, &three_x), &self.b);
        if y2 != rhs { return None; }
        Some(TwPoint { x: xm, y: ym, z: f.one.clone() })
    }
    /// A random private scalar in [1, n - 1], as `size` big-endian bytes.
    fn random_scalar(&self) -> Vec<u8> {
        loop {
            let mut wide = vec![0u8; 2 * self.size];
            tw_fill_random(&mut wide);
            if let Some(k) = self.fnn.reduce_bytes(&wide) {
                if !tw_bn_is_zero(&k) { return tw_bn_to_be(&k, self.size); }
            }
        }
    }
    fn public_key(&self, d: &[u8]) -> Option<Vec<u8>> { self.encode_point(&self.mul_scalar(d, &self.generator())) }
    /// ECDH: the x coordinate of d * peer.
    fn ecdh(&self, d: &[u8], peer: &[u8]) -> Option<Vec<u8>> {
        let q = self.decode_point(peer)?;
        let (x, _) = self.to_affine(&self.mul_scalar(d, &q))?;
        Some(tw_bn_to_be(&x, self.size))
    }
    /// The hash as a scalar: its leftmost `size` bytes, reduced mod n.
    fn hash_scalar(&self, hash: &[u8]) -> Vec<u64> {
        let take = hash.len().min(self.size);
        self.fnn.reduce_bytes(&hash[..take]).unwrap_or_default()
    }
    fn verify(&self, public: &[u8], hash: &[u8], r: &[u8], s: &[u8]) -> bool {
        let n = self.fnn.len();
        let q = match self.decode_point(public) { Some(q) => q, None => return false };
        let (r, s) = match (tw_bn_from_be(r, n), tw_bn_from_be(s, n)) { (Some(r), Some(s)) => (r, s), _ => return false };
        if tw_bn_is_zero(&r) || tw_bn_is_zero(&s) || !tw_bn_lt(&r, &self.fnn.m) || !tw_bn_lt(&s, &self.fnn.m) { return false; }
        let fnn = &self.fnn;
        let e = self.hash_scalar(hash);
        let w = fnn.invert_prime(&fnn.to_mont(&s)); // s^-1 * R
        let u1 = fnn.mul(&w, &e); // e / s, normal form
        let u2 = fnn.mul(&w, &r);
        let p = self.add(&self.mul_scalar(&tw_bn_to_be(&u1, self.size), &self.generator()), &self.mul_scalar(&tw_bn_to_be(&u2, self.size), &q));
        let (x, _) = match self.to_affine(&p) { Some(a) => a, None => return false };
        let xr = fnn.reduce_bytes(&tw_bn_to_be(&x, self.size)).unwrap_or_default();
        xr == r
    }
    /// ECDSA signature (r, s) as big-endian `size`-byte values. The nonce is
    /// derived from fresh randomness, the key and the hash together, so a weak
    /// random source alone does not reveal the key.
    fn sign(&self, d: &[u8], hash: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let fnn = &self.fnn;
        let n = fnn.len();
        let dn = tw_bn_from_be(d, n).unwrap_or_default();
        let e = self.hash_scalar(hash);
        loop {
            let mut seed = vec![0u8; 32];
            tw_fill_random(&mut seed);
            let mut material = seed.clone();
            material.extend_from_slice(d);
            material.extend_from_slice(hash);
            let mut wide = tw_hash(TwHashAlg::Sha512, &material);
            material.push(1);
            wide.extend_from_slice(&tw_hash(TwHashAlg::Sha512, &material));
            let k = match fnn.reduce_bytes(&wide[..2 * self.size]) { Some(k) => k, None => continue };
            if tw_bn_is_zero(&k) { continue; }
            let kb = tw_bn_to_be(&k, self.size);
            let (x, _) = match self.to_affine(&self.mul_scalar(&kb, &self.generator())) { Some(a) => a, None => continue };
            let r = fnn.reduce_bytes(&tw_bn_to_be(&x, self.size)).unwrap_or_default();
            if tw_bn_is_zero(&r) { continue; }
            let kinv = fnn.invert_prime(&fnn.to_mont(&k)); // k^-1 * R
            let rd = fnn.mul(&fnn.to_mont(&r), &dn); // r * d
            let s = fnn.mul(&kinv, &fnn.add(&e, &rd)); // (e + r d) / k
            if tw_bn_is_zero(&s) { continue; }
            return (tw_bn_to_be(&r, self.size), tw_bn_to_be(&s, self.size));
        }
    }
}

// ===========================================================================
// 7. RSA (RFC 8017)
// ===========================================================================

fn tw_strip_zeros(b: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < b.len() && b[i] == 0 { i += 1; }
    &b[i..]
}

/// s^e mod n for a public key; the result has the length of n in bytes.
fn tw_rsa_public(n: &[u8], e: &[u8], sig: &[u8]) -> Option<Vec<u8>> {
    let n = tw_strip_zeros(n);
    let e = tw_strip_zeros(e);
    if n.len() < 128 || n.len() > 1024 || e.is_empty() || e.len() > 8 || n[n.len() - 1] & 1 == 0 { return None; } // 1024 to 8192 bits
    if sig.len() != n.len() { return None; }
    let limbs = (n.len() + 7) / 8;
    let m = TwMont::new(tw_bn_from_be(n, limbs)?);
    let s = tw_bn_from_be(sig, limbs)?;
    if !tw_bn_lt(&s, &m.m) { return None; }
    let r = m.from_mont(&m.pow(&m.to_mont(&s), e));
    Some(tw_bn_to_be(&r, n.len()))
}

fn tw_digest_info_prefix(alg: TwHashAlg) -> Vec<u8> {
    tw_hex_bytes(match alg {
        TwHashAlg::Sha256 => "3031300d060960864801650304020105000420",
        TwHashAlg::Sha384 => "3041300d060960864801650304020205000430",
        TwHashAlg::Sha512 => "3051300d060960864801650304020305000440",
    })
}

fn tw_rsa_verify_pkcs1(n: &[u8], e: &[u8], alg: TwHashAlg, message: &[u8], sig: &[u8]) -> bool {
    let em = match tw_rsa_public(n, e, sig) { Some(m) => m, None => return false };
    let mut t = tw_digest_info_prefix(alg);
    t.extend_from_slice(&tw_hash(alg, message));
    if em.len() < t.len() + 11 { return false; }
    let mut expected = vec![0u8, 1];
    expected.resize(em.len() - t.len() - 1, 0xff);
    expected.push(0);
    expected.extend_from_slice(&t);
    em == expected
}

fn tw_mgf1(alg: TwHashAlg, seed: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut counter = 0u32;
    while out.len() < len {
        let mut m = seed.to_vec();
        m.extend_from_slice(&counter.to_be_bytes());
        out.extend_from_slice(&tw_hash(alg, &m));
        counter += 1;
    }
    out.truncate(len);
    out
}

fn tw_rsa_mod_bits(n: &[u8]) -> usize {
    let n = tw_strip_zeros(n);
    if n.is_empty() { 0 } else { 8 * n.len() - n[0].leading_zeros() as usize }
}

/// RSASSA-PSS verification with MGF1 of the same hash and a salt as long as the hash.
fn tw_rsa_verify_pss(n: &[u8], e: &[u8], alg: TwHashAlg, message: &[u8], sig: &[u8]) -> bool {
    let em_full = match tw_rsa_public(n, e, sig) { Some(m) => m, None => return false };
    let em_bits = tw_rsa_mod_bits(n) - 1;
    let em_len = (em_bits + 7) / 8;
    let em = &em_full[em_full.len() - em_len..];
    if em_full.len() > em_len && em_full[0] != 0 { return false; }
    let h_len = alg.len();
    let s_len = h_len;
    if em_len < h_len + s_len + 2 || em[em_len - 1] != 0xbc { return false; }
    let masked = &em[..em_len - h_len - 1];
    let h = &em[em_len - h_len - 1..em_len - 1];
    let top_bits = 8 * em_len - em_bits;
    if top_bits > 0 && masked[0] >> (8 - top_bits) != 0 { return false; }
    let mask = tw_mgf1(alg, h, masked.len());
    let mut db: Vec<u8> = masked.iter().zip(mask.iter()).map(|(a, b)| a ^ b).collect();
    if top_bits > 0 { db[0] &= 0xff >> top_bits; }
    let ps_len = em_len - h_len - s_len - 2;
    if db[..ps_len].iter().any(|&b| b != 0) || db[ps_len] != 1 { return false; }
    let salt = &db[ps_len + 1..];
    let mut m2 = vec![0u8; 8];
    m2.extend_from_slice(&tw_hash(alg, message));
    m2.extend_from_slice(salt);
    tw_ct_eq(&tw_hash(alg, &m2), h)
}

/// An RSA private key; all numbers big-endian.
#[derive(Clone)]
struct TwRsaKey { n: Vec<u8>, e: Vec<u8>, p: Vec<u8>, q: Vec<u8>, dp: Vec<u8>, dq: Vec<u8>, qinv: Vec<u8> }

/// m^d mod n with the Chinese remainder theorem; constant-time exponentiation.
/// The result is checked with the public key, so a computing fault cannot leak the key.
fn tw_rsa_private(key: &TwRsaKey, m: &[u8]) -> Option<Vec<u8>> {
    let k = tw_strip_zeros(&key.n).len();
    let p = tw_strip_zeros(&key.p);
    let q = tw_strip_zeros(&key.q);
    let (lp, lq) = ((p.len() + 7) / 8, (q.len() + 7) / 8);
    let mp = TwMont::new(tw_bn_from_be(p, lp)?);
    let mq = TwMont::new(tw_bn_from_be(q, lq)?);
    if 2 * lp < (k + 7) / 8 || 2 * lq < (k + 7) / 8 { return None; }
    let m1 = mp.from_mont(&mp.pow(&mp.to_mont(&mp.reduce_bytes(m)?), &key.dp));
    let m2 = mq.from_mont(&mq.pow(&mq.to_mont(&mq.reduce_bytes(m)?), &key.dq));
    let m2p = mp.reduce_bytes(&tw_bn_to_be(&m2, lq * 8))?;
    let diff = mp.sub(&m1, &m2p);
    let qinv = mp.reduce_bytes(&key.qinv)?;
    let h = mp.mul(&mp.to_mont(&qinv), &diff); // qinv * (m1 - m2) mod p
    let hq = tw_bn_mul(&h, &tw_bn_from_be(q, lq)?);
    let (s, _) = tw_bn_add(&hq, &m2);
    let sig = tw_bn_to_be(&s, k);
    let check = tw_rsa_public(&key.n, &key.e, &sig)?;
    let mut padded = vec![0u8; k.saturating_sub(m.len())];
    padded.extend_from_slice(m);
    if !tw_ct_eq(&check, &padded) { return None; }
    Some(sig)
}

/// RSASSA-PSS signature, salt as long as the hash.
fn tw_rsa_sign_pss(key: &TwRsaKey, alg: TwHashAlg, message: &[u8]) -> Option<Vec<u8>> {
    let em_bits = tw_rsa_mod_bits(&key.n) - 1;
    let em_len = (em_bits + 7) / 8;
    let h_len = alg.len();
    if em_len < 2 * h_len + 2 { return None; }
    let mut salt = vec![0u8; h_len];
    tw_fill_random(&mut salt);
    let mut m2 = vec![0u8; 8];
    m2.extend_from_slice(&tw_hash(alg, message));
    m2.extend_from_slice(&salt);
    let h = tw_hash(alg, &m2);
    let mut db = vec![0u8; em_len - 2 * h_len - 2];
    db.push(1);
    db.extend_from_slice(&salt);
    let mask = tw_mgf1(alg, &h, db.len());
    for (d, m) in db.iter_mut().zip(mask.iter()) { *d ^= m; }
    let top_bits = 8 * em_len - em_bits;
    if top_bits > 0 { db[0] &= 0xff >> top_bits; }
    let mut em = db;
    em.extend_from_slice(&h);
    em.push(0xbc);
    tw_rsa_private(key, &em)
}

// ===========================================================================
// 8. Built-ins (section 10 of the specification; std/crypto builds on them)
// ===========================================================================

fn tw_sha512(data: Rc<Vec<u8>>) -> Rc<Vec<u8>> { Rc::new(tw_hash(TwHashAlg::Sha512, &data)) }
fn tw_hmac_sha512(key: Rc<Vec<u8>>, message: Rc<Vec<u8>>) -> Rc<Vec<u8>> { Rc::new(tw_hmac(TwHashAlg::Sha512, &key, &message)) }
fn tw_hkdf_sha256(secret: Rc<Vec<u8>>, salt: Rc<Vec<u8>>, info: Rc<Vec<u8>>, length: i64, loc: &str) -> Rc<Vec<u8>> {
    if !(1..=8160).contains(&length) { t_fail("R004", format!("hkdf_sha256: length {} is outside 1..8160", length), loc) }
    Rc::new(tw_hkdf_expand(TwHashAlg::Sha256, &tw_hkdf_extract(TwHashAlg::Sha256, &salt, &secret), &info, length as usize))
}
fn tw_aead_check(name: &str, key: &[u8], nonce: &[u8], loc: &str) {
    if key.len() != 32 { t_fail("R004", format!("{}: the key must be 32 bytes, not {}", name, key.len()), loc) }
    if nonce.len() != 12 { t_fail("R004", format!("{}: the nonce must be 12 bytes, not {}", name, nonce.len()), loc) }
}
fn tw_aead_seal(key: Rc<Vec<u8>>, nonce: Rc<Vec<u8>>, plaintext: Rc<Vec<u8>>, associated: Rc<Vec<u8>>, loc: &str) -> Rc<Vec<u8>> {
    tw_aead_check("aead_seal", &key, &nonce, loc);
    Rc::new(tw_chacha20_poly1305_seal(&key, &nonce, &associated, &plaintext))
}
fn tw_aead_open(key: Rc<Vec<u8>>, nonce: Rc<Vec<u8>>, sealed: Rc<Vec<u8>>, associated: Rc<Vec<u8>>, loc: &str) -> Option<Rc<Vec<u8>>> {
    tw_aead_check("aead_open", &key, &nonce, loc);
    tw_chacha20_poly1305_open(&key, &nonce, &associated, &sealed).map(Rc::new)
}

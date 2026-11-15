
// ---------------------------------------------------------------------------
// Tests for runtime/crypto.rs and runtime/tls.rs. Concatenated after
// web_test.rs by run_web_tests.sh. Known-answer vectors from the RFCs and
// FIPS documents; signatures and certificates made with OpenSSL.
// ---------------------------------------------------------------------------

const EC_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIBmjCCAUGgAwIBAgIUbRft+RlsEgbrFGfyXiIUT9cGHY4wCgYIKoZIzj0EAwIw
FDESMBAGA1UEAwwJbG9jYWxob3N0MCAXDTI2MTExNTA2MTUwMFoYDzIxMjYxMDIy
MDYxNTAwWjAUMRIwEAYDVQQDDAlsb2NhbGhvc3QwWTATBgcqhkjOPQIBBggqhkjO
PQMBBwNCAAQpVJTPbl6dCoCtgwhfy9BTvexvhmBM62/aR7NgRAGuRk9N0Siuzhdp
05Uph0V5F/wspUr8DPw/QmvAyo2oxsSFo28wbTAdBgNVHQ4EFgQUuWsqgyCIen/8
5TO1dy8Dc9aZ2lEwHwYDVR0jBBgwFoAUuWsqgyCIen/85TO1dy8Dc9aZ2lEwDwYD
VR0TAQH/BAUwAwEB/zAaBgNVHREEEzARgglsb2NhbGhvc3SHBH8AAAEwCgYIKoZI
zj0EAwIDRwAwRAIgbVg4TYdzVoIfZvwXTZOFzwrCoFBSNCdz7adm36NHrXcCIEA5
rI6MXT4+H02RvuJ1xlTonzA7GsgCV2Ky0jR5FsT+
-----END CERTIFICATE-----
"#;
const EC_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgrOePDZte0piJgQZM
WEZOHDXOt7hEC8It8TcYwSofyqOhRANCAAQpVJTPbl6dCoCtgwhfy9BTvexvhmBM
62/aR7NgRAGuRk9N0Siuzhdp05Uph0V5F/wspUr8DPw/QmvAyo2oxsSF
-----END PRIVATE KEY-----
"#;
const RSA_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDJzCCAg+gAwIBAgIUSSeyGzXXiwhL3rmIxC6tvd0MjYwwDQYJKoZIhvcNAQEL
BQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MCAXDTI2MTExNTA2MTUwMVoYDzIxMjYx
MDIyMDYxNTAxWjAUMRIwEAYDVQQDDAlsb2NhbGhvc3QwggEiMA0GCSqGSIb3DQEB
AQUAA4IBDwAwggEKAoIBAQC9BvfIwKPVb3oBLknVOvERyXYKfZ3Zec8pz0gj9t0s
JM2Y1cQhRWxwGu//iLmnmPxYGj7HgyN7ZFT9l2efURka2TbDkwgZ+MJqvHqa2H+L
LQNyn1+Bxols+JjeqQfXzZHhODQR4Llj7YYrrwhpxfYbdKjSCBxqvh+db+oT5tX3
bYg+8Z75ET/xB/xWsmSscPhex7w44RgDYuHBM5O+Qr95+WG6E1Zez1MizJw+psGx
NUP5QCOZo6dv0vN5TyaqYbsb7EtqWwui2xTao+5/Qas+WaY7Q8vPLsYjS+OfXhm1
5bvOXDBDlGJTj9cXx7WSQ6dM4i/e7fu8U0tHtP0AsMT3AgMBAAGjbzBtMB0GA1Ud
DgQWBBRVYYm1jOjLBdLXbTxxhspzQc4o1DAfBgNVHSMEGDAWgBRVYYm1jOjLBdLX
bTxxhspzQc4o1DAPBgNVHRMBAf8EBTADAQH/MBoGA1UdEQQTMBGCCWxvY2FsaG9z
dIcEfwAAATANBgkqhkiG9w0BAQsFAAOCAQEATOjLjZ7Yt6nxDUkdPJAdSD7+VvfO
K7LmwzPWlkknP83arAEV0n5cxDTFpyS69yg3lw0oWbhLc2eqcag4bzEefgs8Xviq
KNxj1SdHGhX+PqJtSSBuwfdz0eLmq6PtB0TNzr05vi5RzjRWeLf5ZFMsqOyaVTcq
TzBEpI2ZD+b40KhZb50eWKjkRp/xmQxIODffznuACUAFhLjCbjI6cL7+87GUKlkS
/H1kXG3eM3kbw0bLZXqOTViVGfH5hJ+ZUxPcHP4dy9OaOsFOffxS4ei6mGs1QA29
aKaFeVPeVZ8r0P9r8HrxnDv16bPnBEEawzE1udIAHUs8bEvJ9HdE4npfiw==
-----END CERTIFICATE-----
"#;
const RSA_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC9BvfIwKPVb3oB
LknVOvERyXYKfZ3Zec8pz0gj9t0sJM2Y1cQhRWxwGu//iLmnmPxYGj7HgyN7ZFT9
l2efURka2TbDkwgZ+MJqvHqa2H+LLQNyn1+Bxols+JjeqQfXzZHhODQR4Llj7YYr
rwhpxfYbdKjSCBxqvh+db+oT5tX3bYg+8Z75ET/xB/xWsmSscPhex7w44RgDYuHB
M5O+Qr95+WG6E1Zez1MizJw+psGxNUP5QCOZo6dv0vN5TyaqYbsb7EtqWwui2xTa
o+5/Qas+WaY7Q8vPLsYjS+OfXhm15bvOXDBDlGJTj9cXx7WSQ6dM4i/e7fu8U0tH
tP0AsMT3AgMBAAECggEAWwxTJ5hakcDbRFxWZeW5Hjcqq0zsTkdU2K+l8OTEjbHA
ciBtExWMcMcY2L2zpx2SVqF1ocXxvkqKgYd++wcmD3+W2Ecm/NOMWbslVxZ3AMOS
yJApganuodVwVLeYNoMnBBN9SSs0QQS0vq/VXpViwMDFXD1Z6rRfKAcy+ZkTcNS0
sqMrZ/tlJaRY1rIHW14jjrCeoNv8VfeaGUCo9bgb9WHCjCM3Kw7x+JDC62l2qEim
KAh11ECehBoEhnPQKHiBlNXwL0tToesilgUFjvxboBelA837/gRJcgAzqsRjKax9
OMabnVn8zvT0cDn3HAwtcCc+RZd4rLEKzpqZL5LnoQKBgQD6vIhmJNWV087/Ry0y
1teuYothcTF6fqOq3+l/6ifQh/C42TCP27ZBJwPip+93BYvZHrb3luCDL7gGErgI
93AVtAWYtBUGmxTSJTfNa/RDfIxOvMk9y+PvvSHTN8BMNMm0fiQv8COlNBp3C/Xx
zmFqLrMukVac4q3oT8Vn34+FEQKBgQDA/s6spqqD8w5cH2JOYJ60+GX+2i/7k1x2
K2oPfq9Py8hRT3S1J7EzVbDWfYxh7FexKNajIP/1WJFjpzCQPyfmWpz6/f/+IILS
9I0+oqSByzL426CM7Rf+axTokeqWFwuK1bsPPXVHZwVNtWYWyuNi6imJImajD1GN
Xnrs+xQJhwKBgDbEdZqyEwEuCbnS5zmCmgvaNI2tNtyFyuQpEozBLb8r4lXOAwB5
Ql8K0ZtxiSnbSUe8oMSF7TZadvzEm/RBb/cdoldPhoQx3aNlQ8IYAXu4OcF9FQvt
fDVCf5/uPo+mvCQzpV3fOzn7nohAUZFmvfp+ht72GhAcp1XmDr14pJWxAoGAJVor
jebD4b3M7a91S04P6V6Z/vUjjEE9B/HnGRCfxYXfQz1pjRYdiZ3wF9M8tEcVwcfx
oqjkYQu/N9ZH4+MVSdQHxdwbC4gnScTjAVhhFm3Jc7iuHhhVKHx5E3oKOPpE1Z4A
EjcG18bsk9mj5I1n7OD9IPlze+OfGQlMKUl+20kCgYEA801n+W6GKTTrfiyMvjmx
XCT8tuHh0E21ECbsfS3kqa5bjGrWPhaze1fv7r4bURCPFBFNM/XrxZAxe2XYNIo2
lXVkRnzON0SwKlgPvVOwrnco1P5CmEVeoQnm7XwdFuKNq/zyoRSyRIZ1V1otZlZU
9UEA980VttKCYTGL6YseJxU=
-----END PRIVATE KEY-----
"#;
const P384_KEY: &str = r#"-----BEGIN EC PRIVATE KEY-----
MIGkAgEBBDAQOcwZcxBL9j/jKG1CyAi9eI3yoNR/tKz9VQ3xR2tSxcISAs46Q7rl
smwqAAXURgagBwYFK4EEACKhZANiAASs0ucmt1cHis5Gw/Kn7PSwERwWEvRmO+RZ
gKvrUwSlcMChJOP/3AjUVPpUQ/4yC2Lu+HXBDddZyBI0onRpdjgTG15qMtlMla14
1l0RM2Pl9OQ19r2K1R4tlfpcF7XiXNo=
-----END EC PRIVATE KEY-----
"#;
const P384_PUB_DER: &str = "3076301006072a8648ce3d020106052b8104002203620004acd2e726b757078ace46c3f2a7ecf4b0111c1612f4663be45980abeb5304a570c0a124e3ffdc08d454fa5443fe320b62eef875c10dd759c81234a274697638131b5e6a32d94c95ad78d65d113363e5f4e435f6bd8ad51e2d95fa5c17b5e25cda";
const EC_SIG: &str = "30450220015ea1fc4edc5291c5c3d8d7fbc9d48deb9614ec8dc4d444b94d23a5ceba2748022100854d47173557f362c23543db7a0d7df8b9ee09b3df2bef86c44082360dbbbca5";
const P384_SIG: &str = "3066023100a73357a298f4d5d2f10bdfc718c7bc9c085c6db03935a338de1f32d2010b69e2f537682cda11a2c6e4a063eec1b7bf56023100fe3ab045aa92d170e1003ce3e297c1531b564fdaae23c9baf1920d363619abdd79d4fb56b6c72484053ae522e6ab68f0";
const RSA_PKCS1_SIG: &str = "2d398f153eac35d32cffb1a4c63647f90ad2ca0a398682fdc3611446e70862364f7ef27117bb746022ebcc496a965993c0f83086db8cffa21a41748a6563505aeb5c2a8c5837f3b902c964686d46f84edc0be43a8fc305daa20fa5f147bd9cb8a202c1fca5ec4b1dbdc9df62cbf9ca7a935850e354f95b5fb1ba2c970bf7f60cc4f35e51c98c66006148e3dc7e87b5a43fb78d220b6014acb7d3d55de827bc857185ed594f35aa8986a3d6f484aff5380cfe56786a6d5cc82ec16ed0a457efb7a23e8d1030abce725ef06bcbbdaae4ed4ecb4afd74efbbe757d322b8c80440a322bba1be43debbbaf1bda42abdd9d1b1b43e8f6317bb8da36d7dbce1adf3ea42";
const RSA_PSS_SIG: &str = "6c9e2719eff65f77aa26a8160c32470b2da2f5487fe9f4128df65b795c61f61b5a7bb2607b2c57cfa4049e8c45d3faaf816dff6e3fd66b68c7920d70f069636a9cdbaee5e66ce01d11e1e208722517047859d295c945e9f83b25215c6f2dd73c330cf664625f7e1381b4154f6cc16133579a5ba8950d4b350f88ad1cfb5232cead232edee5e504175a2c64a6f3214adaa73f45ee8b24ab9de106fc5f4c8c66e97bcf2324ffb9d6af59c09a94b5c9aa0ba2309c76e729c51060f7f3bc91243b6ae99ff85d7ff72a7b396c749c2b346116de1de75d8833883ed654abe3d4e08a6649575d8a8d4318b29914199f1191c616e8cad789e031ccc124bf0e42ac949346";
const MSG: &[u8] = b"hello tessera";

fn hx(s: &str) -> Vec<u8> { tw_hex_bytes(s) }

#[test]
fn sha384_sha512_known_answers() {
    assert_eq!(hex(&tw_hash(TwHashAlg::Sha512, b"abc")), "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f");
    assert_eq!(hex(&tw_hash(TwHashAlg::Sha384, b"abc")), "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7");
    assert_eq!(hex(&tw_hash(TwHashAlg::Sha512, b"")), "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e");
    // two blocks, and an update split across the block boundary
    let long = b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu";
    assert_eq!(hex(&tw_hash(TwHashAlg::Sha512, long)), "8e959b75dae313da8cf4f72814fc143f8f7779c6eb9f7fa17299aeadb6889018501d289e4900f7e4331b99dec4b5433ac7d329eeb6dd26545e96e55b874be909");
    let mut h = TwHasher::new(TwHashAlg::Sha512);
    h.update(&long[..100]); h.update(&long[100..]);
    assert_eq!(h.finish(), tw_hash(TwHashAlg::Sha512, long));
    assert_eq!(tw_hash(TwHashAlg::Sha256, b"abc"), tw_sha256_raw(b"abc").to_vec());
}

#[test]
fn hmac_and_hkdf_known_answers() {
    // RFC 4231 test case 2
    assert_eq!(hex(&tw_hmac(TwHashAlg::Sha512, b"Jefe", b"what do ya want for nothing?")), "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737");
    assert_eq!(hex(&tw_hmac(TwHashAlg::Sha384, b"Jefe", b"what do ya want for nothing?")), "af45d2e376484031617f78d2b58a6b1b9c7ef464f5a01b47e42ec3736322445e8e2240ca5e69e2c78b3239ecfab21649");
    assert_eq!(hex(&tw_hmac(TwHashAlg::Sha256, b"Jefe", b"what do ya want for nothing?")), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    // RFC 4231 test case 6: key longer than the block
    assert_eq!(hex(&tw_hmac(TwHashAlg::Sha512, &[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First")), "80b24263c7c1a3ebb71493c1dd7be8b49b46d1f41b4aeec1121b013783f8f3526b56d037e05f2598bd0fd2215d6a1e5295e64f73f63f0aec8b915a985d786598");
    // RFC 5869 test case 1
    let prk = tw_hkdf_extract(TwHashAlg::Sha256, &hx("000102030405060708090a0b0c"), &[0x0b; 22]);
    assert_eq!(hex(&prk), "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5");
    assert_eq!(hex(&tw_hkdf_expand(TwHashAlg::Sha256, &prk, &hx("f0f1f2f3f4f5f6f7f8f9"), 42)), "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865");
}

#[test]
fn chacha20_poly1305_known_answers() {
    // RFC 8439 section 2.5.2
    let tag = tw_poly1305(&hx("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b"), b"Cryptographic Forum Research Group");
    assert_eq!(hex(&tag), "a8061dc1305136c6c22b8baf0c0127a9");
    // RFC 8439 section 2.8.2
    let key = hx("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f");
    let nonce = hx("070000004041424344454647");
    let aad = hx("50515253c0c1c2c3c4c5c6c7");
    let pt = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
    let sealed = tw_chacha20_poly1305_seal(&key, &nonce, &aad, pt);
    assert_eq!(hex(&sealed), "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d63dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b3692ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc3ff4def08e4b7a9de576d26586cec64b61161ae10b594f09e26a7e902ecbd0600691");
    assert_eq!(tw_chacha20_poly1305_open(&key, &nonce, &aad, &sealed).as_deref(), Some(&pt[..]));
    let mut bad = sealed.clone();
    bad[3] ^= 1;
    assert!(tw_chacha20_poly1305_open(&key, &nonce, &aad, &bad).is_none());
    assert!(tw_chacha20_poly1305_open(&key, &nonce, b"other", &sealed).is_none());
    // Poly1305 edge case: h reaches exactly p before the final reduction (RFC 8439 A.3 #6)
    let tag = tw_poly1305(&hx("0200000000000000000000000000000000000000000000000000000000000000"), &hx("ffffffffffffffffffffffffffffffff"));
    assert_eq!(hex(&tag), "03000000000000000000000000000000");
}

#[test]
fn aes_and_gcm_known_answers() {
    // the S-box, computed rather than stored
    let mut all: Vec<u8> = (0..=255u8).collect();
    for chunk in all.chunks_mut(64) { tw_aes_sub_bytes(chunk); }
    assert_eq!((all[0x00], all[0x01], all[0x53], all[0xff], all[0x10]), (0x63, 0x7c, 0xed, 0x16, 0xca));
    let mut seen = [false; 256];
    for &v in &all { seen[v as usize] = true; }
    assert!(seen.iter().all(|&s| s));
    // FIPS 197 appendix C
    let mut b = [[0u8; 16]];
    b[0].copy_from_slice(&hx("00112233445566778899aabbccddeeff"));
    TwAes::new(&hx("000102030405060708090a0b0c0d0e0f")).encrypt_blocks(&mut b);
    assert_eq!(hex(&b[0]), "69c4e0d86a7b0430d8cdb78070b4c55a");
    b[0].copy_from_slice(&hx("00112233445566778899aabbccddeeff"));
    TwAes::new(&hx("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")).encrypt_blocks(&mut b);
    assert_eq!(hex(&b[0]), "8ea2b7ca516745bfeafc49904b496089");
    // GCM spec (McGrew and Viega) test case 4 and 16
    let pt = hx("d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39");
    let aad = hx("feedfacedeadbeeffeedfacedeadbeefabaddad2");
    let iv = hx("cafebabefacedbaddecaf888");
    let g = TwAesGcm::new(&hx("feffe9928665731c6d6a8f9467308308"));
    let sealed = g.seal(&iv, &aad, &pt);
    assert_eq!(hex(&sealed), "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e0915bc94fbc3221a5db94fae95ae7121a47");
    assert_eq!(g.open(&iv, &aad, &sealed), Some(pt.clone()));
    let mut bad = sealed.clone();
    bad[60] ^= 0x80;
    assert!(g.open(&iv, &aad, &bad).is_none());
    let g256 = TwAesGcm::new(&hx("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308"));
    assert_eq!(hex(&g256.seal(&iv, &aad, &pt)), "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f66276fc6ece0f4e1768cddf8853bb2d551b");
    // empty message (test case 1)
    assert_eq!(hex(&TwAesGcm::new(&[0u8; 16]).seal(&[0u8; 12], &[], &[])), "58e2fccefa7e3061367f1d57a4e7455a");
}

#[test]
fn x25519_known_answers() {
    // RFC 7748 section 5.2 and 6.1
    assert_eq!(hex(&tw_x25519(&hx("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4"), &hx("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c"))), "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552");
    assert_eq!(hex(&tw_x25519(&hx("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d"), &hx("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493"))), "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957");
    let alice = hx("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    let bob = hx("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
    let alice_pub = tw_x25519_base(&alice);
    let bob_pub = tw_x25519_base(&bob);
    assert_eq!(hex(&alice_pub), "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
    assert_eq!(hex(&bob_pub), "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
    assert_eq!(hex(&tw_x25519(&alice, &bob_pub)), "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
    assert_eq!(tw_x25519(&bob, &alice_pub), tw_x25519(&alice, &bob_pub));
}

#[test]
fn montgomery_arithmetic() {
    let m = TwMont::new(tw_bn_from_be(&hx("ffffffff00000001000000000000000000000000ffffffffffffffffffffffff"), 4).unwrap());
    let a = tw_bn_from_be(&hx("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"), 4).unwrap();
    let b = tw_bn_from_be(&hx("fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654320"), 4).unwrap();
    let ab = m.from_mont(&m.mul(&m.to_mont(&a), &m.to_mont(&b)));
    // checked with Python: (a * b) % p
    assert_eq!(hex(&tw_bn_to_be(&ab, 32)), "1ec3536016fa37300f0a8a23ead521ad5a1d217ab14d88842c202ee6b0a916d5");
    let ai = m.invert_prime(&m.to_mont(&a));
    assert_eq!(m.from_mont(&m.mul(&ai, &m.to_mont(&a))), { let mut one = vec![0u64; 4]; one[0] = 1; one });
    // reduce a 512-bit number
    let wide = hx("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    assert_eq!(hex(&tw_bn_to_be(&m.reduce_bytes(&wide).unwrap(), 32)), "00000004fffffffdfffffffffffffffefffffffbffffffff0000000000000002");
}

#[test]
fn ecdsa_and_ecdh() {
    let c = tw_p256();
    // the generator round-trips, n * G is the point at infinity
    let g = c.public_key(&hx("0000000000000000000000000000000000000000000000000000000000000001")).unwrap();
    assert_eq!(hex(&g), "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5");
    assert!(c.to_affine(&c.mul_scalar(&tw_bn_to_be(&c.fnn.m, 32), &c.generator())).is_none());
    // 2G from the textbook
    let g2 = c.public_key(&hx("0000000000000000000000000000000000000000000000000000000000000002")).unwrap();
    assert_eq!(hex(&g2[1..33]), "7cf27b188d034f7e8a52380304b51ac3c08969e277f21b35a60b48fc47669978");
    assert!(c.decode_point(&g2).is_some());
    let mut off = g2.clone();
    off[64] ^= 1;
    assert!(c.decode_point(&off).is_none());
    // ECDH agreement
    let (a, b) = (c.random_scalar(), c.random_scalar());
    let (pa, pb) = (c.public_key(&a).unwrap(), c.public_key(&b).unwrap());
    assert_eq!(c.ecdh(&a, &pb), c.ecdh(&b, &pa));
    // sign and verify, and reject a changed hash
    let h = tw_hash(TwHashAlg::Sha256, MSG);
    let (r, s) = c.sign(&a, &h);
    assert!(c.verify(&pa, &h, &r, &s));
    assert!(!c.verify(&pa, &tw_hash(TwHashAlg::Sha256, b"other"), &r, &s));
    assert!(!c.verify(&pb, &h, &r, &s));
    // OpenSSL-made signatures, verified through the certificate parser
    let cert = tw_parse_cert(&tw_pem_blocks(EC_CERT)[0].1).unwrap();
    assert!(tw_verify_signature(&cert.key, TwSigAlg::Ecdsa(TwHashAlg::Sha256), MSG, &hx(EC_SIG)));
    assert!(!tw_verify_signature(&cert.key, TwSigAlg::Ecdsa(TwHashAlg::Sha256), b"hello tesserA", &hx(EC_SIG)));
    let mut spki = TwDer::new(&hx(P384_PUB_DER)).expect(0x30).unwrap().to_vec();
    let p384 = tw_parse_spki(&spki).unwrap();
    assert!(matches!(p384, TwPubKey::Ec { curve: TwCurveId::P384, .. }));
    assert!(tw_verify_signature(&p384, TwSigAlg::Ecdsa(TwHashAlg::Sha384), MSG, &hx(P384_SIG)));
    spki.clear();
    // our P-384 signature with the OpenSSL key
    let key = tw_parse_ec_private(&tw_pem_blocks(P384_KEY)[0].1, None).unwrap();
    if let TwPrivateKey::Ec { curve, d } = key {
        let h = tw_hash(TwHashAlg::Sha384, MSG);
        let (r, s) = tw_curve(curve).sign(&d, &h);
        assert!(tw_verify_signature(&p384, TwSigAlg::Ecdsa(TwHashAlg::Sha384), MSG, &tw_ecdsa_der_encode(&r, &s)));
    } else { panic!("not an EC key") }
}

#[test]
fn rsa_signatures() {
    let cert = tw_parse_cert(&tw_pem_blocks(RSA_CERT)[0].1).unwrap();
    assert!(tw_verify_signature(&cert.key, TwSigAlg::RsaPkcs1(TwHashAlg::Sha256), MSG, &hx(RSA_PKCS1_SIG)));
    assert!(tw_verify_signature(&cert.key, TwSigAlg::RsaPss(TwHashAlg::Sha256), MSG, &hx(RSA_PSS_SIG)));
    assert!(!tw_verify_signature(&cert.key, TwSigAlg::RsaPss(TwHashAlg::Sha256), MSG, &hx(RSA_PKCS1_SIG)));
    assert!(!tw_verify_signature(&cert.key, TwSigAlg::RsaPkcs1(TwHashAlg::Sha256), b"x", &hx(RSA_PKCS1_SIG)));
    // the self-signed certificate's own signature
    assert!(tw_verify_signature(&cert.key, cert.sig_alg, &cert.tbs, &cert.signature));
    let key = match tw_parse_pkcs8(&tw_pem_blocks(RSA_KEY)[0].1) { Some(TwPrivateKey::Rsa(k)) => k, _ => panic!("not an RSA key") };
    for alg in [TwHashAlg::Sha256, TwHashAlg::Sha384, TwHashAlg::Sha512] {
        let sig = tw_rsa_sign_pss(&key, alg, MSG).unwrap();
        assert!(tw_verify_signature(&cert.key, TwSigAlg::RsaPss(alg), MSG, &sig));
    }
}

#[test]
fn certificates_and_names() {
    let cert = tw_parse_cert(&tw_pem_blocks(EC_CERT)[0].1).unwrap();
    assert_eq!(cert.dns_names, vec!["localhost".to_string()]);
    assert_eq!(cert.ip_addrs, vec![vec![127, 0, 0, 1]]);
    assert!(cert.not_after > tw_now_ms() + 50 * 365 * 86_400_000);
    let anchors = vec![cert.clone()];
    let chain = vec![cert.raw.clone()];
    let now = tw_now_ms();
    assert!(tw_verify_chain(&chain, "localhost", &anchors, now).is_ok());
    assert!(tw_verify_chain(&chain, "LOCALHOST.", &anchors, now).is_ok());
    assert!(tw_verify_chain(&chain, "127.0.0.1", &anchors, now).is_ok());
    let e = tw_verify_chain(&chain, "example.com", &anchors, now).unwrap_err();
    assert!(e.contains("not valid for example.com") && e.contains("localhost, 127.0.0.1"), "{}", e);
    let other = tw_parse_cert(&tw_pem_blocks(RSA_CERT)[0].1).unwrap();
    // same subject name, other key: the signature check rejects it
    let e = tw_verify_chain(&chain, "localhost", &[other], now).unwrap_err();
    assert!(e.contains("signature is invalid"), "{}", e);
    let e = tw_verify_chain(&chain, "localhost", &[], now).unwrap_err();
    assert!(e.contains("not issued by a trusted"), "{}", e);
    let e = tw_verify_chain(&chain, "localhost", &anchors, cert.not_after + 1000).unwrap_err();
    assert!(e.contains("expired"), "{}", e);
    assert!(tw_dns_matches("*.example.com", "www.example.com"));
    assert!(!tw_dns_matches("*.example.com", "example.com"));
    assert!(!tw_dns_matches("*.example.com", "a.b.example.com"));
    assert!(!tw_dns_matches("*.com", "example.com"));
    assert!(!tw_dns_matches("w*.example.com", "www.example.com"));
    assert!(tw_dns_in_subtree("www.example.com", "example.com"));
    assert!(!tw_dns_in_subtree("www.badexample.com", "example.com"));
    assert_eq!(tw_parse_x509_time(0x17, b"491231235959Z"), Some(tw_parse_time(t_text("2049-12-31T23:59:59Z")).unwrap()));
    assert_eq!(tw_parse_x509_time(0x17, b"500101000000Z"), Some(tw_parse_time(t_text("1950-01-01T00:00:00Z")).unwrap()));
    assert_eq!(tw_parse_x509_time(0x18, b"20500101000000Z"), Some(tw_parse_time(t_text("2050-01-01T00:00:00Z")).unwrap()));
    assert!(tw_parse_x509_time(0x17, b"501301000000Z").is_none());
    // a truncated certificate is refused, not misread
    let raw = &cert.raw;
    for cut in [1, 10, raw.len() / 2, raw.len() - 1] { assert!(tw_parse_cert(&raw[..cut]).is_none()); }
}

fn temp_file(name: &str, text: &str) -> String {
    let path = std::env::temp_dir().join(format!("tessera-tls-test-{}-{}", std::process::id(), name));
    std::fs::write(&path, text).unwrap();
    path.to_string_lossy().to_string()
}

/// One TLS connection over loopback: our server, our client, an HTTP-like exchange both ways.
fn loopback(cert: &str, key: &str, host: &str) -> Result<Vec<u8>, String> {
    let config = tw_tls_load_config(&temp_file("c.pem", cert), &temp_file("k.pem", key))?;
    let anchors = tw_certs_from_pem(cert);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || -> Result<(), String> {
        let (sock, _) = listener.accept().unwrap();
        let tls = tw_tls_accept(sock, &config)?;
        let mut s = TwTlsStream::new(tls);
        let mut buf = vec![0u8; 5];
        s.read_exact(&mut buf).map_err(|e| e.to_string())?;
        let big = vec![b'x'; 40000];
        s.write_all(&buf).and_then(|_| s.write_all(&big)).map_err(|e| e.to_string())?;
        s.close();
        Ok(())
    });
    let sock = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let client = tw_tls_connect_with(sock, host, &anchors);
    let out = match client {
        Ok(tls) => {
            let mut c = TwTlsStream::new(tls);
            c.write_all(b"hello").map_err(|e| e.to_string())?;
            let mut got = Vec::new();
            c.read_to_end(&mut got).map_err(|e| e.to_string())?;
            Ok(got)
        }
        Err(e) => Err(e),
    };
    let _ = server.join().unwrap();
    out
}

#[test]
fn tls_loopback_ecdsa_and_rsa() {
    for (cert, key) in [(EC_CERT, EC_KEY), (RSA_CERT, RSA_KEY)] {
        let got = loopback(cert, key, "localhost").unwrap();
        assert_eq!(got.len(), 5 + 40000);
        assert_eq!(&got[..5], b"hello");
    }
    let e = loopback(EC_CERT, EC_KEY, "example.com").unwrap_err();
    assert!(e.contains("not valid for example.com"), "{}", e);
}

#[test]
fn tls_config_errors() {
    let e = tw_tls_load_config(&temp_file("c1.pem", EC_CERT), &temp_file("k1.pem", RSA_KEY)).err().unwrap();
    assert!(e.contains("does not belong"), "{}", e);
    let e = tw_tls_load_config(&temp_file("c2.pem", "nothing"), &temp_file("k2.pem", EC_KEY)).err().unwrap();
    assert!(e.contains("no PEM certificate"), "{}", e);
    let e = tw_tls_load_config(&temp_file("c3.pem", EC_CERT), &temp_file("k3.pem", "-----BEGIN ENCRYPTED PRIVATE KEY-----\nAAAA\n-----END ENCRYPTED PRIVATE KEY-----\n")).err().unwrap();
    assert!(e.contains("encrypted"), "{}", e);
}

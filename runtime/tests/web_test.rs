
// ---------------------------------------------------------------------------
// Tests for runtime/web.rs. Concatenated after core_stub.rs and web.rs by
// run_web_tests.sh and built with `rustc --test`.
// ---------------------------------------------------------------------------

static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn env_lock() -> std::sync::MutexGuard<'static, ()> { TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner()) }

/// Runs `f`, expecting a Tessera runtime failure; returns its code.
fn failure_code<R>(f: impl FnOnce() -> R) -> String {
    TW_PANIC_HOOK.call_once(|| t_install_panic_hook());
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(_) => panic!("expected a runtime failure"),
        Err(p) => match p.downcast_ref::<TFailure>() {
            Some(t) => t.code.clone(),
            None => panic!("expected a TFailure payload"),
        },
    }
}
fn txt(s: &str) -> Rc<String> { t_text(s) }
fn bytes(b: &[u8]) -> Rc<Vec<u8>> { Rc::new(b.to_vec()) }
fn hex(b: &[u8]) -> String { tw_hex_raw(b) }
fn unhex(s: &str) -> Vec<u8> { tw_hex_decode(txt(s)).unwrap().to_vec() }

// ---- 1. decimal ---------------------------------------------------------------

fn dp(s: &str) -> TDecimal { tw_dec_parse(txt(s)).unwrap_or_else(|| panic!("cannot parse {}", s)) }
fn ds(d: TDecimal) -> String { tw_dec_to_text(d).to_string() }
const DMAX: &str = "170141183460469231731687303715.884105727";
const DMIN: &str = "-170141183460469231731687303715.884105728";

#[test]
fn decimal_arithmetic() {
    assert_eq!(ds(tw_dec_add(dp("1.5"), dp("2.25"), "t")), "3.75");
    assert_eq!(ds(tw_dec_sub(dp("1.5"), dp("2.25"), "t")), "-0.75");
    assert_eq!(ds(tw_dec_mul(dp("0.1"), dp("0.2"), "t")), "0.02");
    assert_eq!(ds(tw_dec_mul(dp("-1.5"), dp("2"), "t")), "-3");
    assert_eq!(ds(tw_dec_mul(dp("-1.5"), dp("-2"), "t")), "3");
    assert_eq!(ds(tw_dec_div(dp("1"), dp("3"), "t")), "0.333333333");
    assert_eq!(ds(tw_dec_div(dp("2"), dp("3"), "t")), "0.666666667");
    assert_eq!(ds(tw_dec_div(dp("-2"), dp("3"), "t")), "-0.666666667");
    assert_eq!(ds(tw_dec_div(dp("2"), dp("-3"), "t")), "-0.666666667");
    assert_eq!(ds(tw_dec_div(dp("10"), dp("4"), "t")), "2.5");
    assert_eq!(ds(tw_dec_div(dp("1"), dp("0.000000001"), "t")), "1000000000");
    assert_eq!(ds(tw_dec_div(dp("0.000000001"), dp("2"), "t")), "0.000000001"); // half away from zero
    assert_eq!(ds(tw_dec_div(dp("-0.000000001"), dp("2"), "t")), "-0.000000001");
    assert_eq!(ds(tw_dec_div(dp("0.000000001"), dp("3"), "t")), "0");
    // multiplication rounding of the 10th digit
    assert_eq!(ds(tw_dec_mul(dp("0.000000001"), dp("0.5"), "t")), "0.000000001");
    assert_eq!(ds(tw_dec_mul(dp("-0.000000001"), dp("0.5"), "t")), "-0.000000001");
    assert_eq!(ds(tw_dec_mul(dp("0.000000001"), dp("0.499999999"), "t")), "0");
    assert_eq!(ds(tw_dec_mul(dp("1.000000001"), dp("1.000000001"), "t")), "1.000000002");
    // large values
    assert_eq!(ds(tw_dec_mul(dp("170000000000000000000"), dp("0.5"), "t")), "85000000000000000000");
    assert_eq!(ds(tw_dec_mul(dp("123456789012345678.123456789"), dp("1000"), "t")), "123456789012345678123.456789");
    assert_eq!(ds(tw_dec_mul(dp("100000000000000"), dp("100000000000000"), "t")), "10000000000000000000000000000");
    assert_eq!(ds(tw_dec_mul(dp(DMAX), dp("1"), "t")), DMAX);
    assert_eq!(ds(tw_dec_mul(dp(DMAX), dp("-1"), "t")), "-170141183460469231731687303715.884105727");
    assert_eq!(ds(tw_dec_mul(dp(DMIN), dp("1"), "t")), DMIN);
    assert_eq!(ds(tw_dec_div(dp(DMAX), dp(DMAX), "t")), "1");
    assert_eq!(ds(tw_dec_div(dp(DMIN), dp(DMIN), "t")), "1");
    assert_eq!(ds(tw_dec_div(dp(DMAX), dp("1"), "t")), DMAX);
    assert_eq!(ds(tw_dec_div(dp("1"), dp(DMAX), "t")), "0");
    assert_eq!(ds(tw_dec_div(dp("12345678901234567890.5"), dp("0.5"), "t")), "24691357802469135781");
    // cross-check mul/div against exact i128 arithmetic on small values
    let vals = ["0", "1", "-1", "0.5", "-2.75", "3.141592653", "-0.000000007", "1234.5678", "99999.999999999"];
    for a in vals { for b in vals {
        let (x, y) = (dp(a).0, dp(b).0);
        let p = x * y; // exact at scale 1e18
        let q = p / 1_000_000_000;
        let r = p % 1_000_000_000;
        let expect = if r.abs() * 2 >= 1_000_000_000 { q + p.signum() } else { q };
        assert_eq!(tw_dec_mul(dp(a), dp(b), "t").0, expect, "{} * {}", a, b);
        if y != 0 {
            let n = x * 1_000_000_000;
            let q = n / y;
            let r = n % y;
            let expect = if r.abs() * 2 >= y.abs() { q + if (n < 0) != (y < 0) { -1 } else { 1 } } else { q };
            assert_eq!(tw_dec_div(dp(a), dp(b), "t").0, expect, "{} / {}", a, b);
        }
    } }
}

#[test]
fn decimal_errors() {
    assert_eq!(failure_code(|| tw_dec_add(dp(DMAX), dp("0.000000001"), "t")), "R002");
    assert_eq!(failure_code(|| tw_dec_sub(dp(DMIN), dp("0.000000001"), "t")), "R002");
    assert_eq!(failure_code(|| tw_dec_mul(dp("1000000000000000"), dp("1000000000000000"), "t")), "R002");
    assert_eq!(failure_code(|| tw_dec_mul(dp(DMIN), dp("-1"), "t")), "R002");
    assert_eq!(failure_code(|| tw_dec_mul(dp(DMAX), dp(DMAX), "t")), "R002");
    assert_eq!(failure_code(|| tw_dec_div(dp(DMIN), dp("-1"), "t")), "R002");
    assert_eq!(failure_code(|| tw_dec_div(dp(DMAX), dp("0.5"), "t")), "R002");
    assert_eq!(failure_code(|| tw_dec_div(dp("1"), dp("0"), "t")), "R003");
    assert_eq!(failure_code(|| tw_dec_neg(dp(DMIN), "t")), "R002");
    assert_eq!(ds(tw_dec_neg(dp("1.5"), "t")), "-1.5");
    assert_eq!(ds(tw_dec_neg(dp("0"), "t")), "0");
}

#[test]
fn decimal_conversions_and_rounding() {
    assert_eq!(ds(tw_dec_from_int(i64::MAX)), "9223372036854775807");
    assert_eq!(ds(tw_dec_from_int(i64::MIN)), "-9223372036854775808");
    assert_eq!(tw_dec_to_int(dp("3.9"), "t"), 3);
    assert_eq!(tw_dec_to_int(dp("-3.9"), "t"), -3);
    assert_eq!(tw_dec_to_int(dp("-9223372036854775808.5"), "t"), i64::MIN);
    assert_eq!(failure_code(|| tw_dec_to_int(dp("9223372036854775808"), "t")), "R002");
    assert_eq!(ds(tw_dec_round(dp("2.5"), 0, "t")), "3");
    assert_eq!(ds(tw_dec_round(dp("-2.5"), 0, "t")), "-3");
    assert_eq!(ds(tw_dec_round(dp("2.4999"), 0, "t")), "2");
    assert_eq!(ds(tw_dec_round(dp("1.005"), 2, "t")), "1.01");
    assert_eq!(ds(tw_dec_round(dp("-1.005"), 2, "t")), "-1.01");
    assert_eq!(ds(tw_dec_round(dp("2.449"), 2, "t")), "2.45");
    assert_eq!(ds(tw_dec_round(dp("2.444"), 2, "t")), "2.44");
    assert_eq!(ds(tw_dec_round(dp("0.123456789"), 9, "t")), "0.123456789");
    assert_eq!(ds(tw_dec_round(dp("0.123456789"), 8, "t")), "0.12345679");
    assert_eq!(failure_code(|| tw_dec_round(dp("1"), 10, "t")), "R004");
    assert_eq!(failure_code(|| tw_dec_round(dp("1"), -1, "t")), "R004");
    assert_eq!(failure_code(|| tw_dec_round(dp(DMAX), 0, "t")), "R002");
}

#[test]
fn decimal_text() {
    assert_eq!(ds(dp("3")), "3");
    assert_eq!(ds(dp("1.50")), "1.5");
    assert_eq!(ds(dp("-0.25")), "-0.25");
    assert_eq!(ds(dp("0.000000001")), "0.000000001");
    assert_eq!(ds(dp("-0")), "0");
    assert_eq!(ds(dp("-0.000")), "0");
    assert_eq!(ds(dp("007.50")), "7.5");
    assert_eq!(ds(dp(DMAX)), DMAX);
    assert_eq!(ds(dp(DMIN)), DMIN);
    for bad in ["", "-", "+1", " 1", "1 ", "1.", ".5", "1e5", "1.1234567890", "1..2", "abc", "1,5", "--1", "-.5", "1.2.3", "0x10", "١",
        "170141183460469231731687303716", "170141183460469231731687303715.884105728", "-170141183460469231731687303715.884105729",
        "99999999999999999999999999999999999999999999"] {
        assert!(tw_dec_parse(txt(bad)).is_none(), "{:?} should not parse", bad);
    }
    for _ in 0..2000 {
        let v = TDecimal(((tw_random_u64() as i128) << 64 | tw_random_u64() as i128) >> (tw_random_u64() % 127));
        assert_eq!(tw_dec_parse(tw_dec_to_text(v)), Some(v));
    }
}

// ---- 2. time ------------------------------------------------------------------

fn ft(ms: i64) -> String { tw_format_time(ms).to_string() }
fn pt(s: &str) -> Option<i64> { tw_parse_time(txt(s)) }

#[test]
fn time_format_and_parse() {
    assert_eq!(ft(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(ft(-1), "1969-12-31T23:59:59.999Z");
    assert_eq!(ft(1_000_000_000_000), "2001-09-09T01:46:40.000Z");
    assert_eq!(ft(951_782_400_000), "2000-02-29T00:00:00.000Z");
    assert_eq!(ft(-2_203_891_200_000), "1900-03-01T00:00:00.000Z");
    assert_eq!(ft(-62_167_219_200_000), "0000-01-01T00:00:00.000Z");
    assert_eq!(ft(253_402_300_799_999), "9999-12-31T23:59:59.999Z");
    let t = pt("2026-11-08T14:52:38.123Z").unwrap();
    assert_eq!(ft(t), "2026-11-08T14:52:38.123Z");
    assert_eq!(tw_http_date(t).as_str(), "Sun, 08 Nov 2026 14:52:38 GMT");
    assert_eq!(tw_http_date(0).as_str(), "Thu, 01 Jan 1970 00:00:00 GMT");
    assert_eq!(tw_http_date(784_111_777_000).as_str(), "Sun, 06 Nov 1994 08:49:37 GMT");
    assert_eq!(tw_http_date(-1).as_str(), "Wed, 31 Dec 1969 23:59:59 GMT");
    assert_eq!(pt("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(pt("1970-01-01t00:00:00z"), Some(0));
    assert_eq!(pt("1970-01-01T01:00:00+01:00"), Some(0));
    assert_eq!(pt("1969-12-31T19:00:00-05:00"), Some(0));
    assert_eq!(pt("1969-12-31T23:59:59.999Z"), Some(-1));
    assert_eq!(pt("1970-01-01T00:00:00.5Z"), Some(500));
    assert_eq!(pt("1970-01-01T00:00:00.123999Z"), Some(123));
    assert_eq!(pt("2001-09-09T03:16:40+01:30"), Some(1_000_000_000_000));
    assert!(pt("2024-02-29T00:00:00Z").is_some());
    assert!(pt("2000-02-29T00:00:00Z").is_some());
    for bad in ["2023-02-29T00:00:00Z", "1900-02-29T00:00:00Z", "2024-13-01T00:00:00Z", "2024-00-01T00:00:00Z", "2024-04-31T00:00:00Z",
        "2024-01-01 00:00:00Z", "2024-01-01T00:00:00", "2024-01-01T24:00:00Z", "2024-01-01T00:60:00Z", "2024-01-01T00:00:60Z",
        "2024-01-01T00:00:00+01:60", "2024-01-01T00:00:00+0100", "2024-01-01T00:00:00Z ", "2024-01-01T00:00:00.Z", "24-01-01T00:00:00Z",
        "2024-1-01T00:00:00Z", "", "garbage", "+2024-01-01T00:00:00Z", "2024-01-01T00:00:00+24:00"] {
        assert_eq!(pt(bad), None, "{:?}", bad);
    }
    // round trips over a wide range, including negative times
    let mut ms: i64 = -62_167_219_200_000;
    while ms < 253_402_300_799_999 {
        assert_eq!(pt(&ft(ms)), Some(ms), "{}", ms);
        ms += 7_777_777_777_7 + 123;
    }
    for _ in 0..2000 {
        let ms = (tw_random_u64() % 315_569_520_000_000) as i64 - 62_167_219_200_000;
        assert_eq!(pt(&ft(ms)), Some(ms));
    }
    // extreme values do not panic
    let _ = ft(i64::MIN);
    let _ = ft(i64::MAX);
    let _ = tw_http_date(i64::MIN);
}

#[test]
fn time_calendar_exhaustive() {
    // Walk day by day from 1600-01-01 to 2400-12-31 and compare with a naive calendar.
    let mut days = tw_days_from_civil(1600, 1, 1);
    let (mut y, mut m, mut d) = (1600, 1, 1);
    while y <= 2400 {
        assert_eq!(tw_civil_from_days(days), (y, m, d));
        assert_eq!(tw_days_from_civil(y, m, d), days);
        days += 1;
        d += 1;
        if d > tw_days_in_month(y, m) { d = 1; m += 1; if m > 12 { m = 1; y += 1; } }
    }
    assert_eq!(tw_days_from_civil(1970, 1, 1), 0);
    for z in (-1_000_000..1_000_000).step_by(37) {
        let (y, m, d) = tw_civil_from_days(z);
        assert_eq!(tw_days_from_civil(y, m, d), z);
    }
}

#[test]
fn time_clock() {
    assert!(tw_now_ms() > 1_735_689_600_000); // after 2025-01-01
    let a = tw_monotonic_ms();
    tw_sleep_ms(25, "t");
    let b = tw_monotonic_ms();
    assert!(b - a >= 20, "{} {}", a, b);
    assert_eq!(failure_code(|| tw_sleep_ms(-1, "t")), "R004");
}

// ---- 3. random ----------------------------------------------------------------

#[test]
fn random_ints() {
    assert_eq!(failure_code(|| tw_random_int(5, 5, "t")), "R004");
    assert_eq!(failure_code(|| tw_random_int(6, 5, "t")), "R004");
    let mut counts = [0u32; 10];
    for _ in 0..20000 {
        let v = tw_random_int(0, 10, "t");
        assert!((0..10).contains(&v));
        counts[v as usize] += 1;
    }
    for c in counts { assert!((1600..2400).contains(&c), "{:?}", counts); }
    for _ in 0..1000 {
        let v = tw_random_int(-3, -1, "t");
        assert!(v == -3 || v == -2);
        assert_eq!(tw_random_int(i64::MAX - 1, i64::MAX, "t"), i64::MAX - 1);
        assert_eq!(tw_random_int(i64::MIN, i64::MIN + 1, "t"), i64::MIN);
    }
    let (mut neg, mut pos) = (0, 0);
    for _ in 0..1000 {
        let v = tw_random_int(i64::MIN, i64::MAX, "t");
        if v < 0 { neg += 1 } else { pos += 1 }
    }
    assert!(neg > 400 && pos > 400);
}

#[test]
fn random_bytes_and_chacha() {
    assert_eq!(failure_code(|| tw_random_bytes(-1, "t")), "R004");
    assert_eq!(failure_code(|| tw_random_bytes(1_048_577, "t")), "R004");
    assert_eq!(tw_random_bytes(0, "t").len(), 0);
    assert_eq!(tw_random_bytes(1_048_576, "t").len(), 1_048_576);
    let a = tw_random_bytes(32, "t");
    let b = tw_random_bytes(32, "t");
    assert_eq!(a.len(), 32);
    assert_ne!(a, b);
    // RFC 8439 section 2.3.2 test vector
    let key: Vec<u8> = (0u8..32).collect();
    let kw = tw_words_le(&key);
    let nonce = tw_words_le(&unhex("000000090000004a00000000"));
    let block = tw_chacha20_block(&[kw[0], kw[1], kw[2], kw[3], kw[4], kw[5], kw[6], kw[7]], 1, &[nonce[0], nonce[1], nonce[2]]);
    assert_eq!(hex(&block), "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4ed2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e");
    // the fallback generator: output varies and is roughly balanced
    let mut x = vec![0u8; 4096];
    let mut y = vec![0u8; 4096];
    tw_chacha_fill(&mut x);
    tw_chacha_fill(&mut y);
    assert_ne!(x, y);
    let ones: u32 = x.iter().map(|b| b.count_ones()).sum();
    assert!((15000..17800).contains(&ones), "{}", ones);
    let other = std::thread::spawn(|| { let mut z = vec![0u8; 64]; tw_chacha_fill(&mut z); z }).join().unwrap();
    assert_ne!(&other[..], &x[..64]);
}

// ---- 4. crypto and encodings --------------------------------------------------

#[test]
fn sha256_vectors() {
    let h = |s: &[u8]| hex(&tw_sha256(bytes(s)));
    assert_eq!(h(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(h(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(h(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    assert_eq!(h(b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu"),
        "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1");
    assert_eq!(h(&vec![b'a'; 1_000_000]), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    // incremental updates in odd sizes give the same result
    let data: Vec<u8> = (0..1000u32).map(|i| (i * 7 + 3) as u8).collect();
    for step in [1usize, 3, 55, 56, 63, 64, 65, 127, 999] {
        let mut s = TwSha256::new();
        for c in data.chunks(step) { s.update(c); }
        assert_eq!(s.finish(), tw_sha256_raw(&data));
    }
}

#[test]
fn hmac_sha256_rfc4231() {
    let mac = |k: &[u8], m: &[u8]| hex(&tw_hmac_sha256(bytes(k), bytes(m)));
    assert_eq!(mac(&[0x0b; 20], b"Hi There"), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
    assert_eq!(mac(b"Jefe", b"what do ya want for nothing?"), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    assert_eq!(mac(&[0xaa; 20], &[0xdd; 50]), "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe");
    let k4: Vec<u8> = (1u8..=25).collect();
    assert_eq!(mac(&k4, &[0xcd; 50]), "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b");
    assert_eq!(&mac(&[0x0c; 20], b"Test With Truncation")[..32], "a3b6167473100ee06e0c796c2955552b");
    assert_eq!(mac(&[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First"), "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");
    assert_eq!(mac(&[0xaa; 131], b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm."),
        "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2");
}

#[test]
fn pbkdf2_vectors() {
    assert_eq!(hex(&tw_pbkdf2_sha256(b"password", b"salt", 1, 32)), "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b");
    assert_eq!(hex(&tw_pbkdf2_sha256(b"password", b"salt", 2, 32)), "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43");
    assert_eq!(hex(&tw_pbkdf2_sha256(b"password", b"salt", 4096, 32)), "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a");
    assert_eq!(hex(&tw_pbkdf2_sha256(b"passwordPASSWORDpassword", b"saltSALTsaltSALTsaltSALTsaltSALTsalt", 4096, 40)),
        "348c89dbcbd32b2f32d814b8116e84cf2b17347ebc1800181c4e2a1fb8dd53e1c635518c7dac47e9");
}

#[test]
fn password_hash_and_verify() {
    let h = tw_password_hash(txt("correct horse"), "t");
    let parts: Vec<&str> = h.split('$').collect();
    assert_eq!(parts.len(), 5);
    assert_eq!(parts[1], "pbkdf2-sha256");
    assert_eq!(parts[2], "i=600000");
    assert_eq!(parts[3].len(), 22);
    assert_eq!(parts[4].len(), 43);
    assert!(!parts[3].contains('=') && !parts[4].contains('='));
    assert!(tw_password_verify(txt("correct horse"), h.clone()));
    // low iteration hashes in the same format
    let salt = b"0123456789abcdef";
    let key = tw_pbkdf2_sha256(b"pw", salt, 3, 32);
    let good = format!("$pbkdf2-sha256$i=3${}${}", tw_b64_encode_raw(salt, false), tw_b64_encode_raw(&key, false));
    assert!(tw_password_verify(txt("pw"), txt(&good)));
    assert!(!tw_password_verify(txt("pW"), txt(&good)));
    assert!(!tw_password_verify(txt(""), txt(&good)));
    let s = tw_b64_encode_raw(salt, false);
    let k = tw_b64_encode_raw(&key, false);
    for bad in [String::new(), "$pbkdf2-sha256".to_string(), format!("$pbkdf2-sha256$i=0${}${}", s, k), format!("$pbkdf2-sha256$i=10000001${}${}", s, k),
        format!("$pbkdf2-sha256$i=+3${}${}", s, k), format!("$pbkdf2-sha256$i=3${}${}$", s, k), format!("$pbkdf2-sha1$i=3${}${}", s, k),
        format!("pbkdf2-sha256$i=3${}${}", s, k), format!("$pbkdf2-sha256$3${}${}", s, k), format!("$pbkdf2-sha256$i=3${}$", s),
        format!("$pbkdf2-sha256$i=3$!!!${}", k), format!("$pbkdf2-sha256$i=3${}$*", s), format!("$pbkdf2-sha256$i=4${}${}", s, k),
        format!("$pbkdf2-sha256$i=99999999999999999999${}${}", s, k)] {
        assert!(!tw_password_verify(txt("pw"), txt(&bad)), "{:?}", bad);
    }
}

#[test]
fn base64_and_hex() {
    let vectors = [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")];
    for (plain, enc) in vectors {
        assert_eq!(tw_base64_encode(bytes(plain.as_bytes())).as_str(), enc);
        assert_eq!(tw_base64_decode(txt(enc)).unwrap().as_slice(), plain.as_bytes());
        assert_eq!(tw_base64_decode(txt(enc.trim_end_matches('='))).unwrap().as_slice(), plain.as_bytes());
    }
    assert_eq!(tw_base64_encode(bytes(&[0xfb, 0xff])).as_str(), "+/8=");
    assert_eq!(tw_base64_decode(txt("+/8=")).unwrap().as_slice(), &[0xfb, 0xff]);
    assert_eq!(tw_base64_decode(txt("-_8")).unwrap().as_slice(), &[0xfb, 0xff]);
    assert_eq!(tw_base64_decode(txt("-_8=")).unwrap().as_slice(), &[0xfb, 0xff]);
    for bad in ["Z", "Zg=", "Zg===", "Zm9v=", "Zh==", "Zm9=", "Zm9v Yg==", "=", "==", "Zg==Zg==", "Zm9vY", "Zm\n9v", "Zm9v\n", "é", "Zm9v!"] {
        assert!(tw_base64_decode(txt(bad)).is_none(), "{:?}", bad);
    }
    for n in 0..100 {
        let data = tw_random_bytes(n, "t");
        assert_eq!(tw_base64_decode(tw_base64_encode(data.clone())).unwrap(), data);
        assert_eq!(tw_hex_decode(tw_hex_encode(data.clone())).unwrap(), data);
    }
    assert_eq!(tw_hex_encode(bytes(&[0, 15, 16, 255])).as_str(), "000f10ff");
    assert_eq!(tw_hex_decode(txt("000F10fF")).unwrap().as_slice(), &[0, 15, 16, 255]);
    assert_eq!(tw_hex_decode(txt("")).unwrap().len(), 0);
    for bad in ["abc", "zz", "0g", " 00", "0x00"] { assert!(tw_hex_decode(txt(bad)).is_none(), "{:?}", bad); }
    assert!(tw_equal_constant_time(bytes(b"abc"), bytes(b"abc")));
    assert!(!tw_equal_constant_time(bytes(b"abc"), bytes(b"abd")));
    assert!(!tw_equal_constant_time(bytes(b"abc"), bytes(b"ab")));
    assert!(tw_equal_constant_time(bytes(b""), bytes(b"")));
}

// ---- 5. sqlite ------------------------------------------------------------------

fn params(p: &[&str]) -> Rc<Vec<Rc<String>>> { Rc::new(p.iter().map(|s| txt(s)).collect()) }
fn query(db: &TDatabase, sql: &str, p: &[&str]) -> Result<Vec<std::collections::BTreeMap<String, String>>, String> {
    tw_db_query(db.clone(), txt(sql), params(p), "t")
        .map(|rows| rows.iter().map(|r| r.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()).collect())
        .map_err(|e| e.to_string())
}
fn exec(db: &TDatabase, sql: &str, p: &[&str]) -> Result<i64, String> { tw_db_execute(db.clone(), txt(sql), params(p), "t").map_err(|e| e.to_string()) }

#[test]
fn sqlite_basic_params_nulls_errors() {
    let _g = env_lock();
    let db = tw_db_open(txt(":memory:"), "t").expect("sqlite3 must be installed");
    assert!(db == db.clone());
    assert_eq!(exec(&db, "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT, n INTEGER, r REAL)", &[]), Ok(0));
    let tricky = ["it's", "a\nb", "x'); DROP TABLE t; --", ".shell echo pwned", "\n.quit\n", "héllo ✓", "", "\"quoted\"", "semi;colon", "back\\slash", "\r\n", "?1 ?2", "TESSERA_END_x"];
    for (i, v) in tricky.iter().enumerate() {
        assert_eq!(exec(&db, "INSERT INTO t(id, name, n) VALUES (?1, ?2, ?3)", &[&i.to_string(), v, "42"]), Ok(1));
    }
    let rows = query(&db, "SELECT id, name, n, typeof(n) AS ty FROM t ORDER BY id", &[]).unwrap();
    assert_eq!(rows.len(), tricky.len());
    for (i, v) in tricky.iter().enumerate() {
        assert_eq!(rows[i]["name"], *v);
        assert_eq!(rows[i]["id"], i.to_string());
        assert_eq!(rows[i]["n"], "42");
        assert_eq!(rows[i]["ty"], "integer"); // type affinity converted the text parameter
        assert!(!rows[i].contains_key("r")); // NULL columns are omitted
    }
    let rows = query(&db, "SELECT name FROM t WHERE name = ?1", &["x'); DROP TABLE t; --"]).unwrap();
    assert_eq!(rows.len(), 1);
    let rows = query(&db, "SELECT n + 1 AS m, 0.1 AS f, 1.5 * 2 AS g, NULL AS z, -7 AS neg, 'multi\nline' AS ml, 9223372036854775807 AS big FROM t LIMIT 1", &[]).unwrap();
    assert_eq!(rows[0]["m"], "43");
    assert_eq!(rows[0]["f"], "0.1");
    assert_eq!(rows[0]["g"], "3.0");
    assert_eq!(rows[0]["neg"], "-7");
    assert_eq!(rows[0]["ml"], "multi\nline");
    assert_eq!(rows[0]["big"], "9223372036854775807");
    assert!(!rows[0].contains_key("z"));
    assert_eq!(query(&db, "SELECT * FROM t WHERE id < 0", &[]).unwrap().len(), 0);
    // missing parameters bind NULL
    assert_eq!(query(&db, "SELECT ?1 AS a, ?2 AS b", &["x"]).unwrap()[0].get("b"), None);
    // errors
    let e = query(&db, "SELECT * FROM nope", &[]).unwrap_err();
    assert!(e.contains("no such table: nope"), "{}", e);
    let e = exec(&db, "SELEC 1", &[]).unwrap_err();
    assert!(e.contains("syntax error"), "{}", e);
    let e = exec(&db, "INSERT INTO t(id) VALUES (0)", &[]).unwrap_err();
    assert!(e.contains("UNIQUE constraint failed"), "{}", e);
    // validation of the statement text
    assert!(exec(&db, "SELECT 1; SELECT 2", &[]).unwrap_err().contains("only one SQL statement"));
    assert!(exec(&db, "SELECT 'unterminated", &[]).unwrap_err().contains("unterminated"));
    assert!(exec(&db, "SELECT 1 /* open", &[]).unwrap_err().contains("unterminated"));
    assert!(exec(&db, ".tables", &[]).unwrap_err().contains("'.'"));
    assert!(exec(&db, "-- c\n.shell echo hi", &[]).unwrap_err().contains("'.'"));
    assert!(exec(&db, "SELECT 1\nGO\nSELECT 2", &[]).is_err());
    assert!(exec(&db, "   ", &[]).unwrap_err().contains("empty"));
    // still usable after errors; comments and trailing semicolons are fine
    assert_eq!(query(&db, "  -- leading comment\nSELECT 'a;b' AS x -- trailing comment", &[]).unwrap()[0]["x"], "a;b");
    assert_eq!(query(&db, "SELECT count(*) AS c FROM t;  ", &[]).unwrap()[0]["c"], tricky.len().to_string());
    assert_eq!(query(&db, "SELECT \"name\" AS [x y] FROM t WHERE id = 0;", &[]).unwrap()[0]["x y"], "it's");
    // execute counts
    assert_eq!(exec(&db, "UPDATE t SET r = 1.5", &[]), Ok(tricky.len() as i64));
    assert_eq!(exec(&db, "UPDATE t SET r = 2 WHERE id = ?1", &["3"]), Ok(1));
    assert_eq!(exec(&db, "UPDATE t SET r = 2 WHERE id = -5", &[]), Ok(0));
    assert_eq!(exec(&db, "CREATE TABLE u(x)", &[]), Ok(0)); // DDL reports 0 even after DML
    assert_eq!(exec(&db, "DELETE FROM t WHERE id >= 10", &[]), Ok((tricky.len() - 10) as i64));
    assert_eq!(exec(&db, "SELECT 1", &[]), Ok(0));
    let other = tw_db_open(txt(":memory:"), "t").unwrap();
    assert!(db != other);
    assert!(query(&other, "SELECT * FROM t", &[]).is_err());
}

#[test]
fn sqlite_transactions_files_and_cleanup() {
    let _g = env_lock();
    let dir = std::env::temp_dir().join(format!("tessera-web-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test.db");
    let _ = std::fs::remove_file(&path);
    let p = txt(path.to_str().unwrap());
    let db = tw_db_open(p.clone(), "t").unwrap();
    exec(&db, "CREATE TABLE k(v TEXT)", &[]).unwrap();
    exec(&db, "BEGIN", &[]).unwrap();
    exec(&db, "INSERT INTO k VALUES (?1)", &["rolled back"]).unwrap();
    assert_eq!(query(&db, "SELECT count(*) AS c FROM k", &[]).unwrap()[0]["c"], "1");
    exec(&db, "ROLLBACK", &[]).unwrap();
    assert_eq!(query(&db, "SELECT count(*) AS c FROM k", &[]).unwrap()[0]["c"], "0");
    exec(&db, "BEGIN", &[]).unwrap();
    assert_eq!(exec(&db, "INSERT INTO k VALUES (?1)", &["one"]), Ok(1));
    assert_eq!(exec(&db, "INSERT INTO k VALUES (?1)", &["two"]), Ok(1));
    exec(&db, "COMMIT", &[]).unwrap();
    let pid = db.0.borrow().child.id();
    drop(db);
    if cfg!(target_os = "linux") {
        assert!(!std::path::Path::new(&format!("/proc/{}", pid)).exists(), "sqlite3 child must be gone after drop");
    }
    let db = tw_db_open(p, "t").unwrap();
    let rows = query(&db, "SELECT v FROM k ORDER BY v", &[]).unwrap();
    assert_eq!(rows.iter().map(|r| r["v"].clone()).collect::<Vec<_>>(), vec!["one", "two"]);
    drop(db);
    // a path that cannot be opened gives Err (directory does not exist)
    let missing = dir.join("no/such/dir/x.db");
    let r = tw_db_open(txt(missing.to_str().unwrap()), "t").and_then(|db| tw_db_query(db, txt("CREATE TABLE a(b)"), params(&[]), "t").map(|_| ()));
    assert!(r.is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sqlite_environment() {
    let _g = env_lock();
    std::env::set_var("TESSERA_SQLITE3", "/nonexistent/sqlite3-binary");
    let r = tw_db_open(txt(":memory:"), "t");
    std::env::remove_var("TESSERA_SQLITE3");
    assert!(r.unwrap_err().contains("cannot start"));
    let root = std::env::temp_dir().join(format!("tessera-root-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&root);
    std::env::set_var("TESSERA_FS_ROOT", root.to_str().unwrap());
    let code = failure_code(|| tw_db_open(txt("/etc/x.db"), "t"));
    let code_up = failure_code(|| tw_db_open(txt("../x.db"), "t"));
    let inside = tw_db_open(txt("ok.db"), "t").and_then(|db| tw_db_execute(db, txt("CREATE TABLE a(b)"), params(&[]), "t"));
    let memory = tw_db_open(txt(":memory:"), "t");
    std::env::remove_var("TESSERA_FS_ROOT");
    assert_eq!(code, "R016");
    assert_eq!(code_up, "R016");
    assert_eq!(inside, Ok(0));
    assert!(root.join("ok.db").exists());
    assert!(memory.is_ok());
    drop(inside);
    let _ = std::fs::remove_dir_all(&root);
}

// ---- 6. HTTP client -------------------------------------------------------------

/// A one-connection server: captures the request, sends `response`, closes.
fn one_shot_server(response: Vec<u8>) -> (u16, std::sync::mpsc::Receiver<Vec<u8>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = s.read(&mut buf).unwrap_or(0);
            if n == 0 { break; }
            got.extend_from_slice(&buf[..n]);
            if let Some(p) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&got[..p]).to_ascii_lowercase();
                let cl = head.lines().find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                if got.len() >= p + 4 + cl { break; }
            }
        }
        tx.send(got).unwrap();
        let _ = s.write_all(&response);
    });
    (port, rx)
}
fn client(method: &str, url: &str, headers: &[&str], body: &[u8]) -> Result<String, String> {
    tw_net_request(txt(method), txt(url), params(headers), bytes(body), "t")
        .map(|b| String::from_utf8_lossy(&b).to_string()).map_err(|e| e.to_string())
}

#[test]
fn http_client_requests() {
    let _g = env_lock();
    let (port, rx) = one_shot_server(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhelloEXTRA".to_vec());
    let r = client("GET", &format!("http://127.0.0.1:{}/path?q=1#frag", port), &["X-Test: yes", "Connection: keep-alive"], b"").unwrap();
    assert_eq!(r, "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello");
    let req = String::from_utf8(rx.recv().unwrap()).unwrap();
    assert_eq!(req, format!("GET /path?q=1 HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\nX-Test: yes\r\n\r\n", port));

    let (port, rx) = one_shot_server(b"HTTP/1.1 201 Created\r\nTransfer-Encoding: chunked\r\nX-A: b\r\n\r\n5;ext=1\r\nhello\r\n6\r\n world\r\n0\r\nTrailer: x\r\n\r\n".to_vec());
    let r = client("POST", &format!("http://localhost:{}", port), &["Content-Type: application/json"], b"{\"a\":1}").unwrap();
    assert_eq!(r, "HTTP/1.1 201 Created\r\nX-A: b\r\n\r\nhello world");
    let req = String::from_utf8(rx.recv().unwrap()).unwrap();
    assert!(req.starts_with("POST / HTTP/1.1\r\n"), "{}", req);
    assert!(req.contains("Content-Length: 7\r\n") && req.ends_with("\r\n\r\n{\"a\":1}"), "{}", req);

    let (port, rx) = one_shot_server(b"HTTP/1.1 200 OK\r\n\r\n".to_vec());
    client("PUT", &format!("http://127.0.0.1:{}/x", port), &[], b"").unwrap();
    assert!(String::from_utf8(rx.recv().unwrap()).unwrap().contains("Content-Length: 0\r\n"));

    let (port, _rx) = one_shot_server(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.0 404 Not Found\r\nServer: t\r\n\r\nread to the end".to_vec());
    assert_eq!(client("GET", &format!("http://127.0.0.1:{}/", port), &[], b"").unwrap(), "HTTP/1.1 404 Not Found\r\nServer: t\r\n\r\nread to the end");

    let (port, _rx) = one_shot_server(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n".to_vec());
    assert_eq!(client("HEAD", &format!("http://127.0.0.1:{}/", port), &[], b"").unwrap(), "HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n");

    let (port, _rx) = one_shot_server(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort".to_vec());
    assert!(client("GET", &format!("http://127.0.0.1:{}/", port), &[], b"").unwrap_err().contains("closed"));

    // errors
    assert_eq!(client("GET", "https://example.com/", &[], b"").unwrap_err(), "https is not supported; use a reverse proxy");
    assert!(client("GET", "ftp://example.com/", &[], b"").is_err());
    assert!(client("GET", "http://user:pw@example.com/", &[], b"").is_err());
    assert!(client("GET", "http://127.0.0.1:99999/", &[], b"").is_err());
    assert!(client("GET", "http://127.0.0.1:1/a b", &[], b"").is_err());
    assert!(client("GET", "http://127.0.0.1:1/", &["X: a\r\nInjected: 1"], b"").unwrap_err().contains("invalid header"));
    assert!(client("GET", "http://127.0.0.1:1/", &["no colon"], b"").is_err());
    assert!(client("GE T", "http://127.0.0.1:1/", &[], b"").is_err());
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_port = closed.local_addr().unwrap().port();
    drop(closed);
    assert!(client("GET", &format!("http://127.0.0.1:{}/", closed_port), &[], b"").unwrap_err().contains("cannot connect"));

    // allow list
    std::env::set_var("TESSERA_NET_ALLOW", "example.org, 10.0.0.1:80");
    let denied = client("GET", "http://127.0.0.1:8123/", &[], b"");
    std::env::set_var("TESSERA_NET_ALLOW", "example.org,127.0.0.1");
    let (port, _rx) = one_shot_server(b"HTTP/1.1 204 No Content\r\n\r\n".to_vec());
    let allowed = client("GET", &format!("http://127.0.0.1:{}/", port), &[], b"");
    let (port2, _rx2) = one_shot_server(b"HTTP/1.1 204 No Content\r\n\r\n".to_vec());
    std::env::set_var("TESSERA_NET_ALLOW", format!("127.0.0.1:{}", port2));
    let allowed_port = client("GET", &format!("http://127.0.0.1:{}/", port2), &[], b"");
    let denied_port = client("GET", "http://127.0.0.1:1/", &[], b"");
    std::env::remove_var("TESSERA_NET_ALLOW");
    assert_eq!(denied.unwrap_err(), "network access to 127.0.0.1:8123 is not allowed");
    assert_eq!(allowed.unwrap(), "HTTP/1.1 204 No Content\r\n\r\n");
    assert!(allowed_port.is_ok());
    assert_eq!(denied_port.unwrap_err(), "network access to 127.0.0.1:1 is not allowed");
}

// ---- 7. wire format, run_request -----------------------------------------------

fn sample_request() -> RawRequest {
    RawRequest {
        method: "POST".to_string(), path: "/a%20b/ü".to_string(), query: "x=1&y=2".to_string(), remote_addr: "::1".to_string(),
        headers: vec![("content-type".to_string(), "text/plain".to_string()), ("x-empty".to_string(), String::new())],
        body: vec![0, 1, 2, 255, 10, 13],
    }
}

#[test]
fn wire_format() {
    let req = sample_request();
    let enc = tw_encode_request(&req);
    assert_eq!(&enc[..4], &[1, 0, 0, 0]);
    assert_eq!(tw_decode_request(&enc), Some(req.clone()));
    for i in 0..enc.len() { assert_eq!(tw_decode_request(&enc[..i]), None, "prefix {}", i); }
    let mut longer = enc.clone();
    longer.push(0);
    assert_eq!(tw_decode_request(&longer), None);
    let mut v2 = enc.clone();
    v2[0] = 2;
    assert_eq!(tw_decode_request(&v2), None);
    let resp = RawResponse { status: 404, headers: vec![("a".to_string(), "b".to_string())], body: b"nope".to_vec() };
    let renc = tw_encode_response(&resp);
    assert_eq!(tw_decode_response(&renc), Some(resp.clone()));
    for i in 0..renc.len() { assert_eq!(tw_decode_response(&renc[..i]), None); }
    // explicit layout of a response
    let small = tw_encode_response(&RawResponse { status: 200, headers: vec![], body: b"ok".to_vec() });
    assert_eq!(small, vec![1, 0, 0, 0, 200, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, b'o', b'k']);
    // absurd counts and lengths are rejected without allocating
    let mut huge = vec![1, 0, 0, 0, 200, 0, 0, 0];
    huge.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(tw_decode_response(&huge), None);
    let mut huge_len = vec![1, 0, 0, 0];
    huge_len.extend_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
    assert_eq!(tw_decode_request(&huge_len), None);
    let mut bad_status = vec![1, 0, 0, 0];
    bad_status.extend_from_slice(&70000u32.to_le_bytes());
    bad_status.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(tw_decode_response(&bad_status), None);
    // invalid UTF-8 is decoded lossily
    let mut lossy = Vec::new();
    tw_put_u32(&mut lossy, 1);
    for s in [&b"G\xffT"[..], b"/", b"", b""] { tw_put_bytes(&mut lossy, s); }
    tw_put_u32(&mut lossy, 0);
    tw_put_bytes(&mut lossy, b"");
    assert_eq!(tw_decode_request(&lossy).unwrap().method, "G\u{FFFD}T");
    // random garbage never panics
    for _ in 0..5000 {
        let n = (tw_random_u64() % 64) as usize;
        let mut g = tw_random_bytes(n as i64, "t").to_vec();
        if n >= 4 && tw_random_u64() % 2 == 0 { g[..4].copy_from_slice(&[1, 0, 0, 0]); }
        let _ = tw_decode_request(&g);
        let _ = tw_decode_response(&g);
    }
}

fn test_app(req: RawRequest) -> RawResponse {
    match req.path.as_str() {
        "/fail" => t_fail("R010", "the app failed".to_string(), "app.tess:3:5"),
        "/exit" => std::panic::panic_any(TExit { code: 3 }),
        "/panic" => panic!("plain panic"),
        "/empty" => RawResponse { status: 204, headers: vec![], body: vec![] },
        "/deep" => {
            fn deep(n: u32) -> u64 { let pad = std::hint::black_box([n as u8; 1024]); if n == 0 { pad[0] as u64 } else { deep(n - 1) + pad[1] as u64 } }
            RawResponse { status: 200, headers: vec![], body: deep(20_000).to_string().into_bytes() }
        }
        _ => {
            let x_test = req.headers.iter().find(|(n, _)| n == "x-test").map(|(_, v)| v.clone()).unwrap_or_default();
            let body = format!("method={} path={} query={} body={} x-test={} remote={}", req.method, req.path, req.query, String::from_utf8_lossy(&req.body), x_test, req.remote_addr);
            RawResponse {
                status: 200,
                headers: vec![
                    ("content-type".to_string(), "text/plain".to_string()),
                    ("x-app".to_string(), "1".to_string()),
                    ("x-bad".to_string(), "a\r\nInjected: yes".to_string()),
                    ("Content-Length".to_string(), "999".to_string()),
                    ("Transfer-Encoding".to_string(), "chunked".to_string()),
                    ("Status".to_string(), "302".to_string()),
                ],
                body: body.into_bytes(),
            }
        }
    }
}
fn get(path: &str) -> RawRequest { RawRequest { method: "GET".to_string(), path: path.to_string(), query: String::new(), remote_addr: String::new(), headers: vec![], body: vec![] } }

#[test]
fn run_request_and_library() {
    let ok = tw_run_request(test_app, get("/hello"));
    assert_eq!(ok.status, 200);
    for p in ["/fail", "/exit", "/panic"] {
        let r = tw_run_request(test_app, get(p));
        assert_eq!(r.status, 500, "{}", p);
        assert_eq!(r.body, b"Internal Server Error\n");
        assert_eq!(r.headers[0].0, "content-type");
    }
    // the request thread has a large stack (20 MB of frames here)
    let r = tw_run_request(test_app, get("/deep"));
    assert_eq!(r.status, 200);
    // library protocol
    let out = tw_library_handle(test_app, &tw_encode_request(&sample_request()));
    let resp = tw_decode_response(&out).unwrap();
    assert_eq!(resp.status, 200);
    assert!(String::from_utf8_lossy(&resp.body).starts_with("method=POST path=/a%20b/ü query=x=1&y=2"));
    let bad = tw_decode_response(&tw_library_handle(test_app, b"garbage")).unwrap();
    assert_eq!(bad.status, 400);
    assert_eq!(bad.body, b"Bad Request\n");
    assert_eq!(tw_reason(200), "OK");
    assert_eq!(tw_reason(431), "Request Header Fields Too Large");
    assert_eq!(tw_reason(799), "Unknown");
}

// ---- HTTP server ----------------------------------------------------------------

fn start_server() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || tw_serve_listener(test_app, listener));
    port
}
struct TestResp { status: u16, headers: Vec<(String, String)>, body: Vec<u8> }
impl TestResp {
    fn header(&self, name: &str) -> Option<&str> { self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()) }
    fn count(&self, name: &str) -> usize { self.headers.iter().filter(|(n, _)| n.eq_ignore_ascii_case(name)).count() }
    fn text(&self) -> String { String::from_utf8_lossy(&self.body).to_string() }
}
fn read_resp(r: &mut TwReader<std::net::TcpStream>, head: bool) -> TestResp {
    let line = String::from_utf8(r.read_line(10000).unwrap()).unwrap();
    let status: u16 = line.split(' ').nth(1).unwrap().parse().unwrap();
    assert!(line.starts_with("HTTP/1.1 "));
    let mut headers = Vec::new();
    loop {
        let h = String::from_utf8(r.read_line(10000).unwrap()).unwrap();
        if h.is_empty() { break; }
        let p = h.find(':').unwrap();
        headers.push((h[..p].to_string(), h[p + 1..].trim().to_string()));
    }
    let mut resp = TestResp { status, headers, body: vec![] };
    if !head && status != 100 {
        let n: usize = resp.header("content-length").map(|v| v.parse().unwrap()).unwrap_or(0);
        resp.body = r.read_exact_vec(n).unwrap();
    }
    resp
}
fn connect(port: u16) -> (std::net::TcpStream, TwReader<std::net::TcpStream>) {
    let s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
    let r = TwReader::new(s.try_clone().unwrap());
    (s, r)
}
fn is_closed(r: &mut TwReader<std::net::TcpStream>) -> bool { matches!(r.read_line(100), Err(TwRd::Eof) | Err(TwRd::Io)) }

#[test]
fn http_server_keep_alive_head_chunked() {
    let port = start_server();
    let (mut s, mut r) = connect(port);
    s.write_all(b"GET /hello%20there?a=1&b=2 HTTP/1.1\r\nHost: x\r\nX-Test: one\r\n\r\n").unwrap();
    let a = read_resp(&mut r, false);
    assert_eq!(a.status, 200);
    assert_eq!(a.text(), "method=GET path=/hello%20there query=a=1&b=2 body= x-test=one remote=127.0.0.1");
    assert_eq!(a.header("connection"), Some("keep-alive"));
    assert_eq!(a.header("content-length"), Some(a.body.len().to_string().as_str()));
    assert_eq!(a.count("content-length"), 1);
    assert!(a.header("date").unwrap().ends_with(" GMT"));
    assert_eq!(a.header("x-app"), Some("1"));
    assert_eq!(a.header("x-bad"), None);
    assert_eq!(a.header("injected"), None);
    assert_eq!(a.header("transfer-encoding"), None);
    // second request on the same connection, chunked body
    s.write_all(b"POST /echo HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6;x=y\r\n world\r\n0\r\n\r\n").unwrap();
    let b = read_resp(&mut r, false);
    assert!(b.text().contains("body=hello world"), "{}", b.text());
    // HEAD: content-length but no body, then the connection is still in sync
    s.write_all(b"HEAD /hello HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let h = read_resp(&mut r, true);
    assert_eq!(h.status, 200);
    assert!(h.header("content-length").unwrap().parse::<usize>().unwrap() > 0);
    // pipelined requests in one write; 204 has no body and no content-length
    s.write_all(b"GET /empty HTTP/1.1\r\nHost: x\r\n\r\nPOST /p HTTP/1.1\r\nHost: x\r\nContent-Length: 3\r\n\r\nabc").unwrap();
    let e = read_resp(&mut r, false);
    assert_eq!(e.status, 204);
    assert_eq!(e.header("content-length"), None);
    let p = read_resp(&mut r, false);
    assert!(p.text().contains("method=POST path=/p query= body=abc"));
    // Expect: 100-continue
    s.write_all(b"PUT /up HTTP/1.1\r\nHost: x\r\nContent-Length: 4\r\nExpect: 100-continue\r\n\r\n").unwrap();
    let c = read_resp(&mut r, true);
    assert_eq!(c.status, 100);
    s.write_all(b"data").unwrap();
    assert!(read_resp(&mut r, false).text().contains("body=data"));
    // runtime failure -> 500, connection stays usable
    s.write_all(b"GET /fail HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let f = read_resp(&mut r, false);
    assert_eq!(f.status, 500);
    assert_eq!(f.text(), "Internal Server Error\n");
    // Connection: close
    s.write_all(b"GET /bye HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap();
    let z = read_resp(&mut r, false);
    assert_eq!(z.header("connection"), Some("close"));
    assert!(is_closed(&mut r));
    // HTTP/1.0 closes by default, keeps alive on request
    let (mut s, mut r) = connect(port);
    s.write_all(b"GET /old HTTP/1.0\r\n\r\n").unwrap();
    assert_eq!(read_resp(&mut r, false).header("connection"), Some("close"));
    assert!(is_closed(&mut r));
    let (mut s, mut r) = connect(port);
    s.write_all(b"GET /old HTTP/1.0\r\nConnection: Keep-Alive\r\n\r\nGET /again HTTP/1.0\r\n\r\n").unwrap();
    assert_eq!(read_resp(&mut r, false).header("connection"), Some("keep-alive"));
    assert!(read_resp(&mut r, false).text().contains("path=/again"));
    // absolute-form target
    let (mut s, mut r) = connect(port);
    s.write_all(b"GET http://example.com/abs?q=1 HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n").unwrap();
    assert!(read_resp(&mut r, false).text().contains("path=/abs query=q=1"));
}

#[test]
fn http_server_errors() {
    let port = start_server();
    let send = |raw: &[u8]| -> u16 {
        let (mut s, mut r) = connect(port);
        s.write_all(raw).unwrap();
        let resp = read_resp(&mut r, false);
        if resp.status >= 400 { assert_eq!(resp.header("connection"), Some("close")); assert!(is_closed(&mut r)); }
        resp.status
    };
    assert_eq!(send(b"HELLO\r\n\r\n"), 400);
    assert_eq!(send(b"GET / HTTP/1.1\r\n\r\n"), 400); // no Host
    assert_eq!(send(b"GET  / HTTP/1.1\r\nHost: x\r\n\r\n"), 400);
    assert_eq!(send(b"GET relative HTTP/1.1\r\nHost: x\r\n\r\n"), 400);
    assert_eq!(send(b"GET / HTTP/2.0\r\nHost: x\r\n\r\n"), 505);
    assert_eq!(send(b"GET / HTTP/1.1\r\nHost: x\r\nBad Header: y\r\n\r\n"), 400);
    assert_eq!(send(b"GET / HTTP/1.1\r\nHost: x\r\n folded\r\n\r\n"), 400);
    assert_eq!(send(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 3\r\nTransfer-Encoding: chunked\r\n\r\n"), 400);
    assert_eq!(send(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 3\r\nContent-Length: 4\r\n\r\nabcd"), 400);
    assert_eq!(send(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: -1\r\n\r\n"), 400);
    assert_eq!(send(b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: gzip\r\n\r\n"), 501);
    assert_eq!(send(b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n"), 400);
    assert_eq!(send(b"POST / HTTP/1.1\r\nHost: x\r\nExpect: something\r\n\r\n"), 417);
    assert_eq!(send(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 11000000\r\n\r\n"), 413);
    let mut many = b"GET / HTTP/1.1\r\nHost: x\r\n".to_vec();
    for i in 0..101 { many.extend_from_slice(format!("X-{}: v\r\n", i).as_bytes()); }
    many.extend_from_slice(b"\r\n");
    assert_eq!(send(&many), 431);
    let mut big = b"GET / HTTP/1.1\r\nHost: x\r\nX-Big: ".to_vec();
    big.extend(std::iter::repeat(b'a').take(70_000));
    big.extend_from_slice(b"\r\n\r\n");
    assert_eq!(send(&big), 431);
    // chunked body larger than the limit (10 MiB default)
    let (mut s, mut r) = connect(port);
    s.write_all(b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap();
    let chunk = vec![b'z'; 1024 * 1024];
    let mut status = 0;
    for _ in 0..12 {
        if s.write_all(format!("{:x}\r\n", chunk.len()).as_bytes()).is_err() || s.write_all(&chunk).is_err() || s.write_all(b"\r\n").is_err() { break; }
    }
    if let Ok(line) = r.read_line(1000) { status = String::from_utf8_lossy(&line).split(' ').nth(1).unwrap_or("0").parse().unwrap_or(0); }
    assert_eq!(status, 413);
    // a connection that closes mid-request does not crash the server
    let (mut s, _r) = connect(port);
    s.write_all(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 10\r\n\r\nabc").unwrap();
    drop(s);
    let (mut s, mut r) = connect(port);
    s.write_all(b"GET /still-alive HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    assert_eq!(read_resp(&mut r, false).status, 200);
}

#[test]
fn http_server_concurrent_connections() {
    let port = start_server();
    let handles: Vec<_> = (0..20).map(|i| std::thread::spawn(move || {
        let (mut s, mut r) = connect(port);
        for j in 0..5 {
            s.write_all(format!("GET /c{}-{} HTTP/1.1\r\nHost: x\r\n\r\n", i, j).as_bytes()).unwrap();
            assert!(read_resp(&mut r, false).text().contains(&format!("path=/c{}-{}", i, j)));
        }
    })).collect();
    for h in handles { h.join().unwrap(); }
}

// ---- FastCGI --------------------------------------------------------------------

fn fcgi_params(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (n, v) in pairs { tw_fcgi_encode_pair(&mut out, n.as_bytes(), v.as_bytes()); }
    out
}
fn fcgi_begin(id: u16, role: u16, keep: bool) -> Vec<u8> {
    tw_fcgi_record(1, id, &[(role >> 8) as u8, role as u8, keep as u8, 0, 0, 0, 0, 0])
}
/// Sends one request; returns (stdout bytes, protocol status).
fn fcgi_request(s: &mut std::net::TcpStream, r: &mut std::io::BufReader<std::net::TcpStream>, id: u16, keep: bool, pairs: &[(&str, &str)], body: &[u8]) -> (Vec<u8>, u8) {
    let mut out = fcgi_begin(id, 1, keep);
    let p = fcgi_params(pairs);
    for c in p.chunks(100) { out.extend(tw_fcgi_record(4, id, c)); } // split PARAMS across records
    out.extend(tw_fcgi_record(4, id, &[]));
    for c in body.chunks(65535) { out.extend(tw_fcgi_record(5, id, c)); }
    out.extend(tw_fcgi_record(5, id, &[]));
    s.write_all(&out).unwrap();
    let mut stdout = Vec::new();
    loop {
        let (ty, rid, content) = tw_fcgi_read_record(r).expect("record");
        assert_eq!(rid, id);
        match ty {
            6 => stdout.extend(content),
            3 => return (stdout, content[4]),
            other => panic!("unexpected record type {}", other),
        }
    }
}

#[test]
fn fastcgi_responder() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || tw_fastcgi_serve_listener(test_app, listener));
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
    let mut r = std::io::BufReader::new(s.try_clone().unwrap());
    // management record
    s.write_all(&tw_fcgi_record(9, 0, &fcgi_params(&[("FCGI_MPXS_CONNS", ""), ("FCGI_MAX_CONNS", ""), ("OTHER", "")]))).unwrap();
    let (ty, id, content) = tw_fcgi_read_record(&mut r).unwrap();
    assert_eq!((ty, id), (10, 0));
    let vals = tw_fcgi_decode_pairs(&content).unwrap();
    assert_eq!(vals, vec![(b"FCGI_MPXS_CONNS".to_vec(), b"0".to_vec()), (b"FCGI_MAX_CONNS".to_vec(), b"256".to_vec())]);
    s.write_all(&tw_fcgi_record(99, 0, &[])).unwrap();
    let (ty, _, content) = tw_fcgi_read_record(&mut r).unwrap();
    assert_eq!((ty, content[0]), (11, 99));
    // a request with KEEP_CONN
    let long_value = "v".repeat(300); // 4-byte length encoding
    let (out, proto) = fcgi_request(&mut s, &mut r, 1, true, &[
        ("REQUEST_METHOD", "POST"), ("REQUEST_URI", "/echo%21?x=1"), ("QUERY_STRING", "x=1"), ("CONTENT_LENGTH", "5"),
        ("CONTENT_TYPE", "text/plain"), ("HTTP_X_TEST", "abc"), ("REMOTE_ADDR", "10.0.0.1"), ("HTTP_X_LONG", &long_value)], b"hello");
    assert_eq!(proto, 0);
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("Status: 200 OK\r\n"), "{}", text);
    assert!(text.contains("\r\nx-app: 1\r\n"));
    assert!(!text.contains("Injected") && !text.contains("Status: 302") && !text.contains("999"));
    assert!(text.ends_with("\r\n\r\nmethod=POST path=/echo%21 query=x=1 body=hello x-test=abc remote=10.0.0.1"), "{}", text);
    // second request on the same connection; HEAD sends no body
    let (out, _) = fcgi_request(&mut s, &mut r, 2, true, &[("REQUEST_METHOD", "HEAD"), ("REQUEST_URI", "/h")], b"");
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("Status: 200 OK\r\n") && text.ends_with("\r\n\r\n"), "{}", text);
    // app failure
    let (out, _) = fcgi_request(&mut s, &mut r, 3, true, &[("REQUEST_METHOD", "GET"), ("REQUEST_URI", "/fail")], b"");
    assert!(String::from_utf8(out).unwrap().starts_with("Status: 500 Internal Server Error\r\n"));
    // CONTENT_LENGTH larger than the body sent
    let (out, _) = fcgi_request(&mut s, &mut r, 4, true, &[("REQUEST_METHOD", "POST"), ("REQUEST_URI", "/"), ("CONTENT_LENGTH", "50")], b"short");
    assert!(String::from_utf8(out).unwrap().starts_with("Status: 400 Bad Request\r\n"));
    // unknown role
    s.write_all(&fcgi_begin(5, 2, true)).unwrap();
    let (ty, id, content) = tw_fcgi_read_record(&mut r).unwrap();
    assert_eq!((ty, id, content[4]), (3, 5, 3));
    // abort
    s.write_all(&fcgi_begin(6, 1, true)).unwrap();
    s.write_all(&tw_fcgi_record(2, 6, &[])).unwrap();
    let (ty, id, _) = tw_fcgi_read_record(&mut r).unwrap();
    assert_eq!((ty, id), (3, 6));
    // without KEEP_CONN the connection is closed after the response
    let (out, _) = fcgi_request(&mut s, &mut r, 7, false, &[("REQUEST_METHOD", "GET"), ("PATH_INFO", "/last")], b"");
    assert!(String::from_utf8(out).unwrap().contains("path=/last"));
    assert!(tw_fcgi_read_record(&mut r).is_none());
}

// ---- CGI --------------------------------------------------------------------------

fn vars(v: &[(&str, &str)]) -> Vec<(String, String)> { v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() }

#[test]
fn cgi_request_mapping() {
    let v = vars(&[("REQUEST_METHOD", "POST"), ("REQUEST_URI", "/app/x%20y?q=1"), ("PATH_INFO", "/x y"), ("QUERY_STRING", "q=1"),
        ("REMOTE_ADDR", "192.0.2.1"), ("HTTP_X_FORWARDED_FOR", "1.2.3.4"), ("HTTP_HOST", "example.com"), ("CONTENT_TYPE", "text/plain"),
        ("CONTENT_LENGTH", "3"), ("PATH", "/usr/bin"), ("HTTP_", "ignored")]);
    let mut body: &[u8] = b"abcdef";
    let req = tw_cgi_request(&v, &mut body, 100, false).unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/app/x%20y");
    assert_eq!(req.query, "q=1");
    assert_eq!(req.remote_addr, "192.0.2.1");
    assert_eq!(req.body, b"abc");
    assert_eq!(req.headers, vars(&[("x-forwarded-for", "1.2.3.4"), ("host", "example.com"), ("content-type", "text/plain"), ("content-length", "3")]));
    let mut empty: &[u8] = b"";
    let r = tw_cgi_request(&vars(&[("REQUEST_METHOD", "GET"), ("PATH_INFO", "/info")]), &mut empty, 100, false).unwrap();
    assert_eq!((r.path.as_str(), r.body.len()), ("/info", 0));
    let mut empty: &[u8] = b"";
    assert_eq!(tw_cgi_request(&vars(&[]), &mut empty, 100, false).unwrap().path, "/");
    let mut some: &[u8] = b"leftover";
    assert_eq!(tw_cgi_request(&vars(&[("REQUEST_METHOD", "POST")]), &mut some, 100, false).unwrap().body.len(), 0);
    let mut some: &[u8] = b"leftover";
    assert_eq!(tw_cgi_request(&vars(&[("REQUEST_METHOD", "POST")]), &mut some, 100, true).unwrap().body, b"leftover");
    let mut some: &[u8] = b"leftover";
    assert_eq!(tw_cgi_request(&vars(&[("REQUEST_METHOD", "POST")]), &mut some, 4, true), Err(413));
    let mut short: &[u8] = b"ab";
    assert_eq!(tw_cgi_request(&vars(&[("CONTENT_LENGTH", "3")]), &mut short, 100, false), Err(400));
    let mut short: &[u8] = b"ab";
    assert_eq!(tw_cgi_request(&vars(&[("CONTENT_LENGTH", "x")]), &mut short, 100, false), Err(400));
    let mut short: &[u8] = b"ab";
    assert_eq!(tw_cgi_request(&vars(&[("CONTENT_LENGTH", "101")]), &mut short, 100, false), Err(413));
    let mut short: &[u8] = b"";
    assert_eq!(tw_cgi_request(&vars(&[("REQUEST_METHOD", "G T")]), &mut short, 100, false), Err(400));
    let resp = RawResponse { status: 201, headers: vec![("x-a".to_string(), "1".to_string()), ("Status".to_string(), "500".to_string()), ("x-b".to_string(), "a\nb".to_string())], body: b"made".to_vec() };
    assert_eq!(tw_cgi_response_bytes(&resp, false), b"Status: 201 Created\r\nx-a: 1\r\nContent-Length: 4\r\n\r\nmade".to_vec());
    assert_eq!(tw_cgi_response_bytes(&resp, true), b"Status: 201 Created\r\nx-a: 1\r\nContent-Length: 4\r\n\r\n".to_vec());
    let weird = RawResponse { status: 42, headers: vec![], body: vec![] };
    assert!(tw_cgi_response_bytes(&weird, false).starts_with(b"Status: 500 Internal Server Error\r\n"));
}

#[test]
fn request_command() {
    let args: Vec<String> = ["POST", "/echo?a=b", "-H", "X-Test: hi", "--body", "data"].iter().map(|s| s.to_string()).collect();
    let out = String::from_utf8(tw_request_command(test_app, &args).unwrap()).unwrap();
    assert!(out.starts_with("HTTP/1.1 200 OK\n"), "{}", out);
    assert!(out.contains("\nx-app: 1\n"));
    assert!(out.ends_with("\n\nmethod=POST path=/echo query=a=b body=data x-test=hi remote=127.0.0.1"), "{}", out);
    let fail: Vec<String> = ["GET", "/fail"].iter().map(|s| s.to_string()).collect();
    assert!(String::from_utf8(tw_request_command(test_app, &fail).unwrap()).unwrap().starts_with("HTTP/1.1 500 Internal Server Error\n"));
    for bad in [vec!["GET"], vec!["GET", "/", "-H"], vec!["GET", "/", "-H", "nocolon"], vec!["GET", "/", "--what", "x"], vec!["G T", "/"]] {
        let a: Vec<String> = bad.iter().map(|s| s.to_string()).collect();
        assert!(tw_request_command(test_app, &a).is_none(), "{:?}", bad);
    }
}

#[test]
fn http_server_connection_limit() {
    let port = start_server();
    let mut held = Vec::new();
    for i in 0..TW_MAX_CONNECTIONS {
        let (mut s, mut r) = connect(port);
        s.write_all(format!("GET /hold{} HTTP/1.1\r\nHost: x\r\n\r\n", i).as_bytes()).unwrap();
        assert_eq!(read_resp(&mut r, false).status, 200); // the connection thread is running
        held.push((s, r));
    }
    let (mut s, mut r) = connect(port);
    let _ = s.write_all(b"GET /one-too-many HTTP/1.1\r\nHost: x\r\n\r\n");
    assert_eq!(read_resp(&mut r, false).status, 503);
    // closing one connection frees a slot
    drop(held.pop());
    let mut freed = false;
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let (mut s, mut r) = connect(port);
        let _ = s.write_all(b"GET /again HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
        if read_resp(&mut r, false).status == 200 { freed = true; break; }
    }
    assert!(freed);
}

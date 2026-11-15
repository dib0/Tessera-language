// ---------------------------------------------------------------------------
// Tessera TLS runtime: TLS 1.3 (RFC 8446) for the HTTP client (`https://` in
// `net_request`) and the HTTP server (`serve --tls-cert FILE --tls-key FILE`).
// Pasted at top level after web.rs and crypto.rs. Safe Rust and std only.
//
// Only TLS 1.3: SSL 2/3 and TLS 1.0/1.1 are broken or deprecated (RFC 8996)
// and TLS 1.2 is not implemented. Cipher suites TLS_AES_128_GCM_SHA256,
// TLS_AES_256_GCM_SHA384 and TLS_CHACHA20_POLY1305_SHA256; key exchange
// X25519 or secp256r1; certificates with RSA (PKCS #1 v1.5 and PSS) or ECDSA
// P-256/P-384 keys. No session resumption, 0-RTT or client certificates.
//
// The client checks the server's certificate chain against the system's
// trusted certificates (or the PEM file in TESSERA_CA_FILE), its validity
// dates, key usage, the host name (subjectAltName only) and DNS/IP name
// constraints. There is no switch to turn this off.
//
//   1. DER and PEM
//   2. X.509 certificates and chain validation
//   3. Trusted certificates
//   4. Record layer and key schedule
//   5. Client handshake
//   6. Server handshake and configuration
//   7. Streams for the HTTP code
// ---------------------------------------------------------------------------

// ===========================================================================
// 1. DER and PEM
// ===========================================================================

struct TwDer<'a> { data: &'a [u8], pos: usize }

impl<'a> TwDer<'a> {
    fn new(data: &'a [u8]) -> TwDer<'a> { TwDer { data, pos: 0 } }
    fn is_empty(&self) -> bool { self.pos >= self.data.len() }
    fn peek_tag(&self) -> Option<u8> { self.data.get(self.pos).copied() }
    /// One element: (tag, contents, the whole encoding).
    fn read(&mut self) -> Option<(u8, &'a [u8], &'a [u8])> {
        let start = self.pos;
        let tag = *self.data.get(self.pos)?;
        if tag & 0x1f == 0x1f { return None; } // high tag numbers are not used here
        let first = *self.data.get(self.pos + 1)? as usize;
        let mut p = self.pos + 2;
        let len = if first < 0x80 {
            first
        } else {
            let n = first & 0x7f;
            if n == 0 || n > 3 { return None; }
            let mut l = 0usize;
            for _ in 0..n { l = (l << 8) | *self.data.get(p)? as usize; p += 1; }
            if l < 0x80 || (n > 1 && l < (1 << (8 * (n - 1)))) { return None; } // not minimal
            l
        };
        let end = p.checked_add(len)?;
        if end > self.data.len() { return None; }
        self.pos = end;
        Some((tag, &self.data[p..end], &self.data[start..end]))
    }
    fn expect(&mut self, tag: u8) -> Option<&'a [u8]> {
        let (t, c, _) = self.read()?;
        if t == tag { Some(c) } else { None }
    }
    /// The next element when it has this tag, else nothing is consumed.
    fn optional(&mut self, tag: u8) -> Option<&'a [u8]> {
        if self.peek_tag() == Some(tag) { self.expect(tag) } else { None }
    }
}

/// Positive INTEGER contents without sign padding.
fn tw_der_uint(c: &[u8]) -> Option<Vec<u8>> {
    if c.is_empty() || c[0] & 0x80 != 0 { return None; }
    Some(tw_strip_zeros(c).to_vec())
}
fn tw_der_int_bytes(v: &[u8]) -> Vec<u8> {
    let mut b = tw_strip_zeros(v).to_vec();
    if b.is_empty() || b[0] & 0x80 != 0 { b.insert(0, 0); }
    b
}
fn tw_der_wrap(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let l = content.len();
    if l < 0x80 { out.push(l as u8); } else if l < 0x100 { out.extend_from_slice(&[0x81, l as u8]); } else { out.extend_from_slice(&[0x82, (l >> 8) as u8, l as u8]); }
    out.extend_from_slice(content);
    out
}

/// All PEM blocks of a text: (label, DER bytes).
fn tw_pem_blocks(text: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut label: Option<String> = None;
    let mut body = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("-----BEGIN ") {
            label = Some(rest.trim_end_matches('-').to_string());
            body.clear();
        } else if line.starts_with("-----END ") {
            if let Some(l) = label.take() {
                if let Some(der) = tw_b64_decode_raw(body.as_bytes()) { out.push((l, der)); }
            }
        } else if label.is_some() {
            if line.contains(':') { continue; } // old-style headers (Proc-Type ...)
            body.push_str(line);
        }
    }
    out
}

// ===========================================================================
// 2. X.509 certificates and chain validation (RFC 5280)
// ===========================================================================

const TW_OID_RSA: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
const TW_OID_RSA_PSS: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0a];
const TW_OID_RSA_SHA256: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b];
const TW_OID_RSA_SHA384: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0c];
const TW_OID_RSA_SHA512: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0d];
const TW_OID_EC: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
const TW_OID_P256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
const TW_OID_P384: &[u8] = &[0x2b, 0x81, 0x04, 0x00, 0x22];
const TW_OID_ECDSA_SHA256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02];
const TW_OID_ECDSA_SHA384: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x03];
const TW_OID_ECDSA_SHA512: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x04];
const TW_OID_SHA256: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];
const TW_OID_SHA384: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02];
const TW_OID_SHA512: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x03];
const TW_OID_BASIC_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x13];
const TW_OID_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x0f];
const TW_OID_SAN: &[u8] = &[0x55, 0x1d, 0x11];
const TW_OID_EKU: &[u8] = &[0x55, 0x1d, 0x25];
const TW_OID_NAME_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x1e];
const TW_OID_CERT_POLICIES: &[u8] = &[0x55, 0x1d, 0x20];
const TW_OID_EKU_SERVER: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x01];
const TW_OID_EKU_ANY: &[u8] = &[0x55, 0x1d, 0x25, 0x00];

#[derive(Clone, Copy, PartialEq, Debug)]
enum TwCurveId { P256, P384 }
fn tw_curve(id: TwCurveId) -> &'static TwCurve { match id { TwCurveId::P256 => tw_p256(), TwCurveId::P384 => tw_p384() } }

#[derive(Clone, Debug, PartialEq)]
enum TwPubKey { Rsa { n: Vec<u8>, e: Vec<u8> }, Ec { curve: TwCurveId, point: Vec<u8> }, Unsupported }

#[derive(Clone, Copy, PartialEq, Debug)]
enum TwSigAlg { RsaPkcs1(TwHashAlg), RsaPss(TwHashAlg), Ecdsa(TwHashAlg), Unsupported }

#[derive(Clone, Debug)]
struct TwCert {
    raw: Vec<u8>,
    tbs: Vec<u8>,
    sig_alg: TwSigAlg,
    signature: Vec<u8>,
    issuer: Vec<u8>,
    subject: Vec<u8>,
    not_before: i64,
    not_after: i64,
    key: TwPubKey,
    is_ca: bool,
    path_len: Option<u64>,
    key_usage: Option<u16>,
    /// None when the certificate has no extended key usage.
    server_auth: Option<bool>,
    dns_names: Vec<String>,
    ip_addrs: Vec<Vec<u8>>,
    /// Name constraints: (permitted, excluded); each a list of DNS names or IP (address, mask) pairs.
    constraints: Option<(Vec<TwNameRule>, Vec<TwNameRule>)>,
    unknown_critical: bool,
}

#[derive(Clone, Debug)]
enum TwNameRule { Dns(String), Ip(Vec<u8>, Vec<u8>), Other }

fn tw_parse_sig_alg(c: &[u8]) -> TwSigAlg {
    let mut d = TwDer::new(c);
    let oid = match d.expect(0x06) { Some(o) => o, None => return TwSigAlg::Unsupported };
    let hash_of = |o: &[u8]| if o == TW_OID_SHA256 { Some(TwHashAlg::Sha256) } else if o == TW_OID_SHA384 { Some(TwHashAlg::Sha384) } else if o == TW_OID_SHA512 { Some(TwHashAlg::Sha512) } else { None };
    if oid == TW_OID_RSA_SHA256 { TwSigAlg::RsaPkcs1(TwHashAlg::Sha256) }
    else if oid == TW_OID_RSA_SHA384 { TwSigAlg::RsaPkcs1(TwHashAlg::Sha384) }
    else if oid == TW_OID_RSA_SHA512 { TwSigAlg::RsaPkcs1(TwHashAlg::Sha512) }
    else if oid == TW_OID_ECDSA_SHA256 { TwSigAlg::Ecdsa(TwHashAlg::Sha256) }
    else if oid == TW_OID_ECDSA_SHA384 { TwSigAlg::Ecdsa(TwHashAlg::Sha384) }
    else if oid == TW_OID_ECDSA_SHA512 { TwSigAlg::Ecdsa(TwHashAlg::Sha512) }
    else if oid == TW_OID_RSA_PSS {
        // RSASSA-PSS-params: hash [0], MGF [1] (assumed MGF1 with the same hash), salt length [2]
        let params = match d.expect(0x30) { Some(p) => p, None => return TwSigAlg::Unsupported };
        let mut p = TwDer::new(params);
        let mut hash = None;
        let mut salt = 20u64;
        while let Some((tag, content, _)) = p.read() {
            match tag {
                0xa0 => { let mut h = TwDer::new(content); hash = h.expect(0x30).and_then(|a| TwDer::new(a).expect(0x06)).and_then(hash_of); }
                0xa2 => { salt = TwDer::new(content).expect(0x02).map(|v| v.iter().fold(0u64, |a, b| (a << 8) | *b as u64)).unwrap_or(0); }
                _ => {}
            }
        }
        match hash { Some(h) if salt == h.len() as u64 => TwSigAlg::RsaPss(h), _ => TwSigAlg::Unsupported }
    } else { TwSigAlg::Unsupported }
}

fn tw_parse_spki(c: &[u8]) -> Option<TwPubKey> {
    let mut d = TwDer::new(c);
    let alg = d.expect(0x30)?;
    let bits = d.expect(0x03)?;
    if bits.first() != Some(&0) { return None; }
    let key = &bits[1..];
    let mut a = TwDer::new(alg);
    let oid = a.expect(0x06)?;
    if oid == TW_OID_RSA {
        let mut k = TwDer::new(TwDer::new(key).expect(0x30)?);
        let n = tw_der_uint(k.expect(0x02)?)?;
        let e = tw_der_uint(k.expect(0x02)?)?;
        Some(TwPubKey::Rsa { n, e })
    } else if oid == TW_OID_EC {
        let curve = a.expect(0x06)?;
        let id = if curve == TW_OID_P256 { TwCurveId::P256 } else if curve == TW_OID_P384 { TwCurveId::P384 } else { return Some(TwPubKey::Unsupported) };
        Some(TwPubKey::Ec { curve: id, point: key.to_vec() })
    } else {
        Some(TwPubKey::Unsupported)
    }
}

/// UTCTime or GeneralizedTime to milliseconds since 1970.
fn tw_parse_x509_time(tag: u8, c: &[u8]) -> Option<i64> {
    let s = std::str::from_utf8(c).ok()?;
    if !s.ends_with('Z') || !s[..s.len() - 1].bytes().all(|b| b.is_ascii_digit()) { return None; }
    let (year, rest) = match (tag, s.len()) {
        (0x17, 13) => { let y: i64 = s[..2].parse().ok()?; (if y < 50 { 2000 + y } else { 1900 + y }, &s[2..]) }
        (0x18, 15) => (s[..4].parse().ok()?, &s[4..]),
        _ => return None,
    };
    let n = |i: usize| -> Option<i64> { rest[i..i + 2].parse().ok() };
    let (mo, d, h, mi, se) = (n(0)?, n(2)?, n(4)?, n(6)?, n(8)?);
    if !(1..=12).contains(&mo) || d < 1 || d > tw_days_in_month(year, mo) || h > 23 || mi > 59 || se > 60 { return None; }
    Some(((tw_days_from_civil(year, mo, d) * 24 + h) * 60 + mi) * 60_000 + se * 1000)
}

fn tw_parse_general_names(c: &[u8], cert: &mut TwCert) -> Option<()> {
    let mut d = TwDer::new(c);
    while !d.is_empty() {
        let (tag, v, _) = d.read()?;
        match tag {
            0x82 => cert.dns_names.push(std::str::from_utf8(v).ok()?.to_ascii_lowercase()),
            0x87 => cert.ip_addrs.push(v.to_vec()),
            _ => {}
        }
    }
    Some(())
}

fn tw_parse_name_rules(c: &[u8]) -> Option<Vec<TwNameRule>> {
    let mut out = Vec::new();
    let mut d = TwDer::new(c);
    while !d.is_empty() {
        let subtree = d.expect(0x30)?;
        let mut s = TwDer::new(subtree);
        let (tag, v, _) = s.read()?;
        if !s.is_empty() { out.push(TwNameRule::Other); continue; } // minimum/maximum are not used in practice
        out.push(match tag {
            0x82 => TwNameRule::Dns(std::str::from_utf8(v).ok()?.to_ascii_lowercase()),
            0x87 if v.len() == 8 || v.len() == 32 => TwNameRule::Ip(v[..v.len() / 2].to_vec(), v[v.len() / 2..].to_vec()),
            _ => TwNameRule::Other,
        });
    }
    Some(out)
}

fn tw_parse_cert(raw: &[u8]) -> Option<TwCert> {
    let mut outer = TwDer::new(raw);
    let body = outer.expect(0x30)?;
    if !outer.is_empty() { return None; }
    let mut c = TwDer::new(body);
    let (tbs_tag, tbs_content, tbs_raw) = c.read()?;
    if tbs_tag != 0x30 { return None; }
    let sig_alg = tw_parse_sig_alg(c.expect(0x30)?);
    let sig_bits = c.expect(0x03)?;
    if sig_bits.first() != Some(&0) { return None; }
    let mut t = TwDer::new(tbs_content);
    let version = match t.optional(0xa0) { Some(v) => *TwDer::new(v).expect(0x02)?.last()?, None => 0 };
    t.expect(0x02)?; // serial
    t.expect(0x30)?; // signature algorithm (repeated)
    let (it, _, issuer) = t.read()?;
    let validity = t.expect(0x30)?;
    let (st, _, subject) = t.read()?;
    let (pt, spki, _) = t.read()?;
    if it != 0x30 || st != 0x30 || pt != 0x30 { return None; }
    let mut v = TwDer::new(validity);
    let (nbt, nb, _) = v.read()?;
    let (nat, na, _) = v.read()?;
    let mut cert = TwCert {
        raw: raw.to_vec(), tbs: tbs_raw.to_vec(), sig_alg, signature: sig_bits[1..].to_vec(),
        issuer: issuer.to_vec(), subject: subject.to_vec(),
        not_before: tw_parse_x509_time(nbt, nb)?, not_after: tw_parse_x509_time(nat, na)?,
        key: tw_parse_spki(spki)?, is_ca: false, path_len: None, key_usage: None, server_auth: None,
        dns_names: Vec::new(), ip_addrs: Vec::new(), constraints: None, unknown_critical: false,
    };
    t.optional(0x81); // issuerUniqueID
    t.optional(0x82); // subjectUniqueID
    if let Some(ext_wrap) = t.optional(0xa3) {
        if version != 2 { return None; }
        let mut exts = TwDer::new(TwDer::new(ext_wrap).expect(0x30)?);
        while !exts.is_empty() {
            let mut e = TwDer::new(exts.expect(0x30)?);
            let oid = e.expect(0x06)?;
            let critical = match e.optional(0x01) { Some(b) => b == [0xff], None => false };
            let value = e.expect(0x04)?;
            if oid == TW_OID_BASIC_CONSTRAINTS {
                let mut b = TwDer::new(TwDer::new(value).expect(0x30)?);
                if let Some(ca) = b.optional(0x01) { cert.is_ca = ca == [0xff]; }
                if let Some(pl) = b.optional(0x02) { cert.path_len = Some(pl.iter().fold(0u64, |a, x| (a << 8) | *x as u64)); }
            } else if oid == TW_OID_KEY_USAGE {
                let bits = TwDer::new(value).expect(0x03)?;
                let b0 = *bits.get(1).unwrap_or(&0) as u16;
                let b1 = *bits.get(2).unwrap_or(&0) as u16;
                cert.key_usage = Some((b0 << 8) | b1);
            } else if oid == TW_OID_SAN {
                tw_parse_general_names(TwDer::new(value).expect(0x30)?, &mut cert)?;
            } else if oid == TW_OID_EKU {
                let mut k = TwDer::new(TwDer::new(value).expect(0x30)?);
                let mut ok = false;
                while !k.is_empty() { let o = k.expect(0x06)?; if o == TW_OID_EKU_SERVER || o == TW_OID_EKU_ANY { ok = true; } }
                cert.server_auth = Some(ok);
            } else if oid == TW_OID_NAME_CONSTRAINTS {
                let mut n = TwDer::new(TwDer::new(value).expect(0x30)?);
                let permitted = match n.optional(0xa0) { Some(p) => tw_parse_name_rules(p)?, None => Vec::new() };
                let excluded = match n.optional(0xa1) { Some(x) => tw_parse_name_rules(x)?, None => Vec::new() };
                cert.constraints = Some((permitted, excluded));
            } else if oid == TW_OID_CERT_POLICIES {
                // policies are not evaluated (no policy is required), which RFC 5280 allows
            } else if critical {
                cert.unknown_critical = true;
            }
        }
    }
    Some(cert)
}

/// ECDSA-Sig-Value (DER) to (r, s).
fn tw_ecdsa_der_decode(sig: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut d = TwDer::new(sig);
    let body = d.expect(0x30)?;
    if !d.is_empty() { return None; }
    let mut b = TwDer::new(body);
    let r = tw_der_uint(b.expect(0x02)?)?;
    let s = tw_der_uint(b.expect(0x02)?)?;
    if !b.is_empty() { return None; }
    Some((r, s))
}
fn tw_ecdsa_der_encode(r: &[u8], s: &[u8]) -> Vec<u8> {
    let mut body = tw_der_wrap(0x02, &tw_der_int_bytes(r));
    body.extend_from_slice(&tw_der_wrap(0x02, &tw_der_int_bytes(s)));
    tw_der_wrap(0x30, &body)
}

fn tw_verify_signature(key: &TwPubKey, alg: TwSigAlg, message: &[u8], sig: &[u8]) -> bool {
    match (key, alg) {
        (TwPubKey::Rsa { n, e }, TwSigAlg::RsaPkcs1(h)) => tw_rsa_mod_bits(n) >= 2048 && tw_rsa_verify_pkcs1(n, e, h, message, sig),
        (TwPubKey::Rsa { n, e }, TwSigAlg::RsaPss(h)) => tw_rsa_mod_bits(n) >= 2048 && tw_rsa_verify_pss(n, e, h, message, sig),
        (TwPubKey::Ec { curve, point }, TwSigAlg::Ecdsa(h)) => match tw_ecdsa_der_decode(sig) {
            Some((r, s)) => tw_curve(*curve).verify(point, &tw_hash(h, message), &r, &s),
            None => false,
        },
        _ => false,
    }
}

fn tw_dns_matches(pattern: &str, host: &str) -> bool {
    let pattern = pattern.trim_end_matches('.');
    if let Some(suffix) = pattern.strip_prefix("*.") {
        if !suffix.contains('.') || suffix.contains('*') { return false; }
        match host.find('.') { Some(p) => p > 0 && &host[p + 1..] == suffix, None => false }
    } else {
        !pattern.contains('*') && pattern == host
    }
}

fn tw_host_ip(host: &str) -> Option<Vec<u8>> {
    match host.parse::<std::net::IpAddr>().ok()? {
        std::net::IpAddr::V4(a) => Some(a.octets().to_vec()),
        std::net::IpAddr::V6(a) => Some(a.octets().to_vec()),
    }
}

/// Is `name` inside the DNS subtree `rule` ("example.com" covers the name and its subdomains; ".example.com" only subdomains)?
fn tw_dns_in_subtree(name: &str, rule: &str) -> bool {
    let name = name.trim_start_matches("*.");
    if rule.is_empty() { return true; }
    if let Some(r) = rule.strip_prefix('.') { return name.ends_with(&format!(".{}", r)); }
    name == rule || name.ends_with(&format!(".{}", rule))
}

fn tw_check_constraints(ca: &TwCert, leaf: &TwCert) -> Result<(), String> {
    let (permitted, excluded) = match &ca.constraints { Some(c) => c, None => return Ok(()) };
    if permitted.iter().chain(excluded.iter()).any(|r| matches!(r, TwNameRule::Other)) {
        return Err("a CA certificate has name constraints of a kind that is not supported".to_string());
    }
    let ip_in = |ip: &[u8], net: &[u8], mask: &[u8]| ip.len() == net.len() && ip.iter().zip(net.iter()).zip(mask.iter()).all(|((a, n), m)| a & m == n & m);
    for name in &leaf.dns_names {
        if excluded.iter().any(|r| matches!(r, TwNameRule::Dns(d) if tw_dns_in_subtree(name, d))) { return Err(format!("the name {} is excluded by a CA's name constraints", name)); }
        let rules: Vec<&String> = permitted.iter().filter_map(|r| if let TwNameRule::Dns(d) = r { Some(d) } else { None }).collect();
        if !rules.is_empty() && !rules.iter().any(|d| tw_dns_in_subtree(name, d)) { return Err(format!("the name {} is outside a CA's name constraints", name)); }
    }
    for ip in &leaf.ip_addrs {
        if excluded.iter().any(|r| matches!(r, TwNameRule::Ip(n, m) if ip_in(ip, n, m))) { return Err("an IP address is excluded by a CA's name constraints".to_string()); }
        let rules: Vec<(&Vec<u8>, &Vec<u8>)> = permitted.iter().filter_map(|r| if let TwNameRule::Ip(n, m) = r { Some((n, m)) } else { None }).collect();
        if !rules.is_empty() && !rules.iter().any(|(n, m)| ip_in(ip, n, m)) { return Err("an IP address is outside a CA's name constraints".to_string()); }
    }
    Ok(())
}

const TW_KU_DIGITAL_SIGNATURE: u16 = 0x8000;
const TW_KU_KEY_CERT_SIGN: u16 = 0x0400;

fn tw_check_validity(cert: &TwCert, now: i64, what: &str) -> Result<(), String> {
    if now < cert.not_before { return Err(format!("the {} certificate is not valid yet (from {})", what, tw_format_time(cert.not_before))); }
    if now > cert.not_after { return Err(format!("the {} certificate has expired (on {})", what, tw_format_time(cert.not_after))); }
    if cert.unknown_critical { return Err(format!("the {} certificate has a critical extension that is not supported", what)); }
    if cert.server_auth == Some(false) { return Err(format!("the {} certificate is not meant for TLS servers (extended key usage)", what)); }
    Ok(())
}

/// Checks the chain the server sent (leaf first) for `host` against `anchors`.
fn tw_verify_chain(chain: &[Vec<u8>], host: &str, anchors: &[TwCert], now: i64) -> Result<(), String> {
    let first = chain.first().ok_or_else(|| "the server sent no certificate".to_string())?;
    let leaf = tw_parse_cert(first).ok_or_else(|| "the server certificate cannot be read".to_string())?;
    tw_check_validity(&leaf, now, "server")?;
    if let Some(ku) = leaf.key_usage { if ku & TW_KU_DIGITAL_SIGNATURE == 0 { return Err("the server certificate may not be used for signatures (key usage)".to_string()); } }
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let name_ok = match tw_host_ip(&host) {
        Some(ip) => leaf.ip_addrs.iter().any(|a| *a == ip),
        None => leaf.dns_names.iter().any(|p| tw_dns_matches(p, &host)),
    };
    if !name_ok {
        let names: Vec<String> = leaf.dns_names.iter().cloned().chain(leaf.ip_addrs.iter().filter_map(|a| match a.len() {
            4 => Some(std::net::Ipv4Addr::new(a[0], a[1], a[2], a[3]).to_string()),
            16 => { let mut b = [0u8; 16]; b.copy_from_slice(a); Some(std::net::Ipv6Addr::from(b).to_string()) }
            _ => None,
        })).collect();
        return Err(format!("the server certificate is not valid for {} (it names: {})", host, if names.is_empty() { "nothing".to_string() } else { names.join(", ") }));
    }
    if anchors.iter().any(|a| a.raw == leaf.raw) { return Ok(()); }
    let intermediates: Vec<TwCert> = chain[1..].iter().filter_map(|c| tw_parse_cert(c)).collect();
    let mut last_err = "the server certificate was not issued by a trusted certificate authority".to_string();
    if tw_build_path(&leaf, &leaf, &intermediates, anchors, now, 0, &mut last_err) { Ok(()) } else { Err(last_err) }
}

fn tw_issuer_ok(ca: &TwCert, child: &TwCert, leaf: &TwCert, depth: usize, now: i64, is_anchor: bool) -> Result<(), String> {
    if !is_anchor {
        tw_check_validity(ca, now, "intermediate")?;
        if !ca.is_ca { return Err("an intermediate certificate is not a CA certificate".to_string()); }
    } else if now > ca.not_after || now < ca.not_before {
        return Err("the trusted root certificate is outside its validity period".to_string());
    }
    if let Some(ku) = ca.key_usage { if ku & TW_KU_KEY_CERT_SIGN == 0 { return Err("a CA certificate may not sign certificates (key usage)".to_string()); } }
    if let Some(pl) = ca.path_len { if (depth as u64) > pl { return Err("the certificate chain is longer than a CA allows (path length)".to_string()); } }
    if child.sig_alg == TwSigAlg::Unsupported { return Err("a certificate uses a signature algorithm that is not supported".to_string()); }
    if !tw_verify_signature(&ca.key, child.sig_alg, &child.tbs, &child.signature) { return Err("a certificate signature is invalid".to_string()); }
    tw_check_constraints(ca, leaf)
}

/// Depth-first search from `child` up to a trust anchor; `depth` counts the CA certificates below.
fn tw_build_path(child: &TwCert, leaf: &TwCert, intermediates: &[TwCert], anchors: &[TwCert], now: i64, depth: usize, err: &mut String) -> bool {
    if depth > 8 { *err = "the certificate chain is too long".to_string(); return false; }
    for a in anchors.iter().filter(|a| a.subject == child.issuer) {
        match tw_issuer_ok(a, child, leaf, depth, now, true) { Ok(()) => return true, Err(e) => *err = e }
    }
    for c in intermediates.iter().filter(|c| c.subject == child.issuer && c.raw != child.raw) {
        match tw_issuer_ok(c, child, leaf, depth, now, false) {
            Ok(()) => { if tw_build_path(c, leaf, intermediates, anchors, now, depth + 1, err) { return true; } }
            Err(e) => *err = e,
        }
    }
    false
}

// ===========================================================================
// 3. Trusted certificates
// ===========================================================================

static TW_TRUST: std::sync::OnceLock<Result<Vec<TwCert>, String>> = std::sync::OnceLock::new();

fn tw_certs_from_pem(text: &str) -> Vec<TwCert> {
    tw_pem_blocks(text).into_iter().filter(|(l, _)| l == "CERTIFICATE" || l == "TRUSTED CERTIFICATE").filter_map(|(_, der)| tw_parse_cert(&der)).collect()
}

#[cfg(windows)]
fn tw_system_roots() -> Result<Vec<TwCert>, String> {
    // The Windows certificate store is reachable only through CryptoAPI (FFI,
    // which generated code may not use), so PowerShell exports it.
    let script = "$ErrorActionPreference='SilentlyContinue'; foreach ($s in 'Cert:\\LocalMachine\\Root','Cert:\\LocalMachine\\AuthRoot','Cert:\\CurrentUser\\Root') { Get-ChildItem $s | ForEach-Object { [Convert]::ToBase64String($_.RawData) } }";
    let out = std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", script]).output()
        .map_err(|e| format!("cannot read the Windows certificate store through PowerShell: {}", e))?;
    let certs: Vec<TwCert> = String::from_utf8_lossy(&out.stdout).lines()
        .filter_map(|l| tw_b64_decode_raw(l.trim().as_bytes())).filter_map(|der| tw_parse_cert(&der)).collect();
    if certs.is_empty() { return Err("the Windows certificate store has no usable root certificates; set TESSERA_CA_FILE to a PEM file".to_string()); }
    Ok(certs)
}

#[cfg(not(windows))]
fn tw_system_roots() -> Result<Vec<TwCert>, String> {
    let mut paths: Vec<String> = Vec::new();
    if let Ok(p) = std::env::var("SSL_CERT_FILE") { if !p.is_empty() { paths.push(p); } }
    for p in ["/etc/ssl/certs/ca-certificates.crt", "/etc/pki/tls/certs/ca-bundle.crt", "/etc/ssl/ca-bundle.pem", "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem", "/etc/ssl/cert.pem", "/usr/local/etc/openssl/cert.pem", "/opt/homebrew/etc/openssl@3/cert.pem"] {
        paths.push(p.to_string());
    }
    for p in &paths {
        if let Ok(text) = std::fs::read_to_string(p) {
            let certs = tw_certs_from_pem(&text);
            if !certs.is_empty() { return Ok(certs); }
        }
    }
    Err("no trusted certificates found (looked for the system CA bundle); set TESSERA_CA_FILE to a PEM file".to_string())
}

/// The trust anchors: TESSERA_CA_FILE when set (instead of the system's), else the system's. Loaded once.
fn tw_trust_anchors() -> Result<&'static Vec<TwCert>, String> {
    TW_TRUST.get_or_init(|| {
        match std::env::var("TESSERA_CA_FILE") {
            Ok(p) if !p.is_empty() => {
                let text = std::fs::read_to_string(&p).map_err(|e| format!("cannot read TESSERA_CA_FILE {}: {}", p, e))?;
                let certs = tw_certs_from_pem(&text);
                if certs.is_empty() { Err(format!("TESSERA_CA_FILE {} holds no certificates", p)) } else { Ok(certs) }
            }
            _ => tw_system_roots(),
        }
    }).as_ref().map_err(|e| e.clone())
}

// ===========================================================================
// 4. Record layer and key schedule
// ===========================================================================

const TW_CT_CCS: u8 = 20;
const TW_CT_ALERT: u8 = 21;
const TW_CT_HANDSHAKE: u8 = 22;
const TW_CT_APP: u8 = 23;

const TW_HS_CLIENT_HELLO: u8 = 1;
const TW_HS_SERVER_HELLO: u8 = 2;
const TW_HS_NEW_SESSION_TICKET: u8 = 4;
const TW_HS_ENCRYPTED_EXTENSIONS: u8 = 8;
const TW_HS_CERTIFICATE: u8 = 11;
const TW_HS_CERTIFICATE_REQUEST: u8 = 13;
const TW_HS_CERTIFICATE_VERIFY: u8 = 15;
const TW_HS_FINISHED: u8 = 20;
const TW_HS_KEY_UPDATE: u8 = 24;

const TW_SUITE_AES128: u16 = 0x1301;
const TW_SUITE_AES256: u16 = 0x1302;
const TW_SUITE_CHACHA: u16 = 0x1303;
const TW_GROUP_X25519: u16 = 0x001d;
const TW_GROUP_P256: u16 = 0x0017;

const TW_SIG_ECDSA_P256: u16 = 0x0403;
const TW_SIG_ECDSA_P384: u16 = 0x0503;
const TW_SIG_PSS_256: u16 = 0x0804;
const TW_SIG_PSS_384: u16 = 0x0805;
const TW_SIG_PSS_512: u16 = 0x0806;
const TW_SIG_PKCS1_256: u16 = 0x0401;
const TW_SIG_PKCS1_384: u16 = 0x0501;
const TW_SIG_PKCS1_512: u16 = 0x0601;

const TW_ALERT_CLOSE_NOTIFY: u8 = 0;
const TW_ALERT_UNEXPECTED_MESSAGE: u8 = 10;
const TW_ALERT_BAD_RECORD_MAC: u8 = 20;
const TW_ALERT_HANDSHAKE_FAILURE: u8 = 40;
const TW_ALERT_BAD_CERTIFICATE: u8 = 42;
const TW_ALERT_ILLEGAL_PARAMETER: u8 = 47;
const TW_ALERT_UNKNOWN_CA: u8 = 48;
const TW_ALERT_DECODE_ERROR: u8 = 50;
const TW_ALERT_DECRYPT_ERROR: u8 = 51;
const TW_ALERT_PROTOCOL_VERSION: u8 = 70;
const TW_ALERT_INTERNAL_ERROR: u8 = 80;
const TW_ALERT_MISSING_EXTENSION: u8 = 109;
const TW_ALERT_NO_APPLICATION_PROTOCOL: u8 = 120;

const TW_HRR_RANDOM: [u8; 32] = [
    0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65, 0xb8, 0x91,
    0xc2, 0xa2, 0x11, 0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2, 0xc8, 0xa8, 0x33, 0x9c,
];

fn tw_alert_name(code: u8) -> &'static str {
    match code {
        0 => "close_notify", 10 => "unexpected_message", 20 => "bad_record_mac", 22 => "record_overflow",
        40 => "handshake_failure", 42 => "bad_certificate", 43 => "unsupported_certificate", 44 => "certificate_revoked",
        45 => "certificate_expired", 46 => "certificate_unknown", 47 => "illegal_parameter", 48 => "unknown_ca",
        49 => "access_denied", 50 => "decode_error", 51 => "decrypt_error", 70 => "protocol_version",
        71 => "insufficient_security", 80 => "internal_error", 86 => "inappropriate_fallback", 90 => "user_canceled",
        109 => "missing_extension", 110 => "unsupported_extension", 112 => "unrecognized_name", 116 => "certificate_required",
        120 => "no_application_protocol", _ => "unknown alert",
    }
}

fn tw_suite_hash(suite: u16) -> TwHashAlg { if suite == TW_SUITE_AES256 { TwHashAlg::Sha384 } else { TwHashAlg::Sha256 } }

#[derive(Clone)]
enum TwAead { Aes(TwAesGcm), ChaCha(Vec<u8>) }

#[derive(Clone)]
struct TwKeys { aead: TwAead, iv: [u8; 12], seq: u64 }

fn tw_expand_label(alg: TwHashAlg, secret: &[u8], label: &str, context: &[u8], len: usize) -> Vec<u8> {
    let full = format!("tls13 {}", label);
    let mut info = Vec::with_capacity(4 + full.len() + context.len());
    info.extend_from_slice(&(len as u16).to_be_bytes());
    info.push(full.len() as u8);
    info.extend_from_slice(full.as_bytes());
    info.push(context.len() as u8);
    info.extend_from_slice(context);
    tw_hkdf_expand(alg, secret, &info, len)
}

fn tw_traffic_keys(suite: u16, secret: &[u8]) -> TwKeys {
    let alg = tw_suite_hash(suite);
    let key_len = if suite == TW_SUITE_AES128 { 16 } else { 32 };
    let key = tw_expand_label(alg, secret, "key", &[], key_len);
    let ivv = tw_expand_label(alg, secret, "iv", &[], 12);
    let mut iv = [0u8; 12];
    iv.copy_from_slice(&ivv);
    let aead = if suite == TW_SUITE_CHACHA { TwAead::ChaCha(key) } else { TwAead::Aes(TwAesGcm::new(&key)) };
    TwKeys { aead, iv, seq: 0 }
}

impl TwKeys {
    fn nonce(&self) -> [u8; 12] {
        let mut n = self.iv;
        for (i, b) in self.seq.to_be_bytes().iter().enumerate() { n[4 + i] ^= b; }
        n
    }
    fn seal(&mut self, aad: &[u8], pt: &[u8]) -> Vec<u8> {
        let n = self.nonce();
        self.seq += 1;
        match &self.aead { TwAead::Aes(g) => g.seal(&n, aad, pt), TwAead::ChaCha(k) => tw_chacha20_poly1305_seal(k, &n, aad, pt) }
    }
    fn open(&mut self, aad: &[u8], ct: &[u8]) -> Option<Vec<u8>> {
        let n = self.nonce();
        self.seq += 1;
        match &self.aead { TwAead::Aes(g) => g.open(&n, aad, ct), TwAead::ChaCha(k) => tw_chacha20_poly1305_open(k, &n, aad, ct) }
    }
}

/// A TLS 1.3 connection over a TCP stream, after or during the handshake.
struct TwTls {
    sock: std::net::TcpStream,
    suite: u16,
    read_keys: Option<TwKeys>,
    write_keys: Option<TwKeys>,
    read_secret: Vec<u8>,
    write_secret: Vec<u8>,
    raw: Vec<u8>,
    hs: Vec<u8>,
    app: Vec<u8>,
    app_pos: usize,
    peer_closed: bool,
    sent_close: bool,
    transcript: Vec<u8>,
    handshake_done: bool,
}

/// A failure with the alert to send for it.
struct TwTlsErr { alert: u8, message: String }
fn tw_tls_err(alert: u8, message: impl Into<String>) -> TwTlsErr { TwTlsErr { alert, message: message.into() } }

const TW_MAX_HANDSHAKE_MESSAGE: usize = 256 * 1024;

impl TwTls {
    fn new(sock: std::net::TcpStream) -> TwTls {
        TwTls { sock, suite: 0, read_keys: None, write_keys: None, read_secret: Vec::new(), write_secret: Vec::new(), raw: Vec::new(), hs: Vec::new(), app: Vec::new(), app_pos: 0, peer_closed: false, sent_close: false, transcript: Vec::new(), handshake_done: false }
    }
    fn hash(&self) -> TwHashAlg { tw_suite_hash(self.suite) }
    fn transcript_hash(&self) -> Vec<u8> { tw_hash(self.hash(), &self.transcript) }

    fn send_raw(&mut self, data: &[u8]) -> Result<(), TwTlsErr> {
        self.sock.write_all(data).map_err(|e| tw_tls_err(TW_ALERT_INTERNAL_ERROR, format!("cannot send: {}", e)))
    }
    fn write_record(&mut self, ctype: u8, data: &[u8]) -> Result<(), TwTlsErr> {
        let mut out = Vec::with_capacity(data.len() + 64);
        let chunks: Vec<&[u8]> = if data.is_empty() { vec![data] } else { data.chunks(16384).collect() };
        for chunk in chunks {
            match self.write_keys.as_mut() {
                None => {
                    out.extend_from_slice(&[ctype, 3, if ctype == TW_CT_HANDSHAKE && chunk.first() == Some(&TW_HS_CLIENT_HELLO) { 1 } else { 3 }]);
                    out.extend_from_slice(&(chunk.len() as u16).to_be_bytes());
                    out.extend_from_slice(chunk);
                }
                Some(keys) => {
                    let mut inner = chunk.to_vec();
                    inner.push(ctype);
                    let len = (inner.len() + 16) as u16;
                    let header = [TW_CT_APP, 3, 3, (len >> 8) as u8, len as u8];
                    out.extend_from_slice(&header);
                    out.extend_from_slice(&keys.seal(&header, &inner));
                }
            }
        }
        self.send_raw(&out)
    }
    fn send_handshake(&mut self, msg: &[u8]) -> Result<(), TwTlsErr> {
        self.transcript.extend_from_slice(msg);
        self.write_record(TW_CT_HANDSHAKE, msg)
    }
    fn send_alert(&mut self, code: u8) {
        if self.sent_close { return; }
        self.sent_close = true;
        let level = if code == TW_ALERT_CLOSE_NOTIFY { 1 } else { 2 };
        let _ = self.write_record(TW_CT_ALERT, &[level, code]);
        let _ = self.sock.flush();
    }
    fn fill(&mut self, need: usize) -> Result<bool, TwTlsErr> {
        let mut tmp = [0u8; 16384 + 512];
        while self.raw.len() < need {
            match self.sock.read(&mut tmp) {
                Ok(0) => return Ok(false),
                Ok(n) => self.raw.extend_from_slice(&tmp[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(tw_tls_err(TW_ALERT_INTERNAL_ERROR, format!("cannot receive: {}", e))),
            }
        }
        Ok(true)
    }
    /// The next record as (content type, plaintext); None at a clean end of the stream.
    fn read_record(&mut self) -> Result<Option<(u8, Vec<u8>)>, TwTlsErr> {
        loop {
            if !self.fill(5)? {
                if self.raw.is_empty() { return Ok(None); }
                return Err(tw_tls_err(TW_ALERT_DECODE_ERROR, "the connection was closed in the middle of a record"));
            }
            let ctype = self.raw[0];
            let len = ((self.raw[3] as usize) << 8) | self.raw[4] as usize;
            if self.raw[1] != 3 { return Err(tw_tls_err(TW_ALERT_DECODE_ERROR, "not a TLS record (is this an https server?)")); }
            if len > 16384 + 256 { return Err(tw_tls_err(22, "record too long")); }
            if !self.fill(5 + len)? { return Err(tw_tls_err(TW_ALERT_DECODE_ERROR, "the connection was closed in the middle of a record")); }
            let header: Vec<u8> = self.raw[..5].to_vec();
            let body: Vec<u8> = self.raw[5..5 + len].to_vec();
            self.raw.drain(..5 + len);
            if ctype == TW_CT_CCS {
                if self.handshake_done || body != [1] { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "unexpected change_cipher_spec")); }
                continue; // compatibility record, ignored (RFC 8446 section 5)
            }
            let (ctype, plain) = match self.read_keys.as_mut() {
                None => (ctype, body),
                Some(keys) => {
                    if ctype != TW_CT_APP { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "unencrypted record after the key exchange")); }
                    let mut inner = keys.open(&header, &body).ok_or_else(|| tw_tls_err(TW_ALERT_BAD_RECORD_MAC, "a record failed authentication (bad_record_mac)"))?;
                    while inner.last() == Some(&0) { inner.pop(); }
                    let t = inner.pop().ok_or_else(|| tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "record without content type"))?;
                    if inner.len() > 16384 { return Err(tw_tls_err(22, "record too long")); }
                    (t, inner)
                }
            };
            if ctype == TW_CT_ALERT {
                if plain.len() != 2 { return Err(tw_tls_err(TW_ALERT_DECODE_ERROR, "malformed alert")); }
                if plain[1] == TW_ALERT_CLOSE_NOTIFY { self.peer_closed = true; return Ok(None); }
                self.sent_close = true; // no alert in reply to a fatal one
                return Err(tw_tls_err(TW_ALERT_CLOSE_NOTIFY, format!("the peer sent the TLS alert {} ({})", tw_alert_name(plain[1]), plain[1])));
            }
            if ctype != TW_CT_HANDSHAKE && ctype != TW_CT_APP { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, format!("unexpected record type {}", ctype))); }
            if ctype == TW_CT_HANDSHAKE && plain.is_empty() { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "empty handshake record")); }
            return Ok(Some((ctype, plain)));
        }
    }
    /// The next whole handshake message (header included), during the handshake.
    fn read_handshake(&mut self) -> Result<Vec<u8>, TwTlsErr> {
        loop {
            if self.hs.len() >= 4 {
                let len = ((self.hs[1] as usize) << 16) | ((self.hs[2] as usize) << 8) | self.hs[3] as usize;
                if len > TW_MAX_HANDSHAKE_MESSAGE { return Err(tw_tls_err(TW_ALERT_DECODE_ERROR, "handshake message too long")); }
                if self.hs.len() >= 4 + len {
                    let msg: Vec<u8> = self.hs.drain(..4 + len).collect();
                    return Ok(msg);
                }
            }
            match self.read_record()? {
                None => return Err(tw_tls_err(TW_ALERT_DECODE_ERROR, "the connection was closed during the handshake")),
                Some((TW_CT_HANDSHAKE, data)) => self.hs.extend_from_slice(&data),
                Some(_) => return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "application data during the handshake")),
            }
        }
    }
    fn expect_handshake(&mut self, kind: u8, what: &str) -> Result<Vec<u8>, TwTlsErr> {
        let msg = self.read_handshake()?;
        if msg[0] != kind { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, format!("expected {}, got handshake message {}", what, msg[0]))); }
        Ok(msg)
    }
    fn finished_data(&self, base_secret: &[u8]) -> Vec<u8> {
        let alg = self.hash();
        let key = tw_expand_label(alg, base_secret, "finished", &[], alg.len());
        tw_hmac(alg, &key, &self.transcript_hash())
    }
    fn set_read_secret(&mut self, secret: Vec<u8>) { self.read_keys = Some(tw_traffic_keys(self.suite, &secret)); self.read_secret = secret; }
    fn set_write_secret(&mut self, secret: Vec<u8>) { self.write_keys = Some(tw_traffic_keys(self.suite, &secret)); self.write_secret = secret; }
    fn update_read(&mut self) {
        let next = tw_expand_label(self.hash(), &self.read_secret, "traffic upd", &[], self.hash().len());
        self.set_read_secret(next);
    }
    fn update_write(&mut self) {
        let next = tw_expand_label(self.hash(), &self.write_secret, "traffic upd", &[], self.hash().len());
        self.set_write_secret(next);
    }

    /// Application data after the handshake; handles post-handshake messages. 0 at the end.
    fn read_app(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.app_pos < self.app.len() {
                let n = out.len().min(self.app.len() - self.app_pos);
                out[..n].copy_from_slice(&self.app[self.app_pos..self.app_pos + n]);
                self.app_pos += n;
                if self.app_pos == self.app.len() { self.app.clear(); self.app_pos = 0; }
                return Ok(n);
            }
            if self.peer_closed { return Ok(0); }
            match self.read_record() {
                Ok(None) => {
                    if self.peer_closed { return Ok(0); }
                    // closed without close_notify: possibly a truncation attack
                    return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "TLS connection closed without close_notify"));
                }
                Ok(Some((TW_CT_APP, data))) => { self.app = data; self.app_pos = 0; }
                Ok(Some((_, data))) => {
                    self.hs.extend_from_slice(&data);
                    while self.hs.len() >= 4 {
                        let len = ((self.hs[1] as usize) << 16) | ((self.hs[2] as usize) << 8) | self.hs[3] as usize;
                        if len > TW_MAX_HANDSHAKE_MESSAGE { self.send_alert(TW_ALERT_DECODE_ERROR); return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "TLS message too long")); }
                        if self.hs.len() < 4 + len { break; }
                        let msg: Vec<u8> = self.hs.drain(..4 + len).collect();
                        match msg[0] {
                            TW_HS_NEW_SESSION_TICKET => {} // resumption is not supported
                            TW_HS_KEY_UPDATE if len == 1 && msg[4] <= 1 => {
                                self.update_read();
                                if msg[4] == 1 {
                                    if self.write_record(TW_CT_HANDSHAKE, &[TW_HS_KEY_UPDATE, 0, 0, 1, 0]).is_err() { return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "TLS send failed")); }
                                    self.update_write();
                                }
                            }
                            _ => { self.send_alert(TW_ALERT_UNEXPECTED_MESSAGE); return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "unexpected TLS handshake message")); }
                        }
                    }
                }
                Err(e) => {
                    let alert = e.alert;
                    if alert != TW_ALERT_CLOSE_NOTIFY { self.send_alert(alert); }
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e.message));
                }
            }
        }
    }
    fn write_app(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if data.is_empty() { return Ok(0); }
        self.write_record(TW_CT_APP, data).map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.message))?;
        Ok(data.len())
    }
}

/// Handshake message: type, 24-bit length, body.
fn tw_hs_msg(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut m = vec![kind, (body.len() >> 16) as u8, (body.len() >> 8) as u8, body.len() as u8];
    m.extend_from_slice(body);
    m
}
fn tw_put_u16(out: &mut Vec<u8>, v: usize) { out.extend_from_slice(&(v as u16).to_be_bytes()); }
fn tw_put_u24(out: &mut Vec<u8>, v: usize) { out.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, v as u8]); }
fn tw_ext(out: &mut Vec<u8>, kind: u16, body: &[u8]) { tw_put_u16(out, kind as usize); tw_put_u16(out, body.len()); out.extend_from_slice(body); }

/// Reader for handshake message bodies; every failure is a decode_error.
struct TwBuf<'a> { d: &'a [u8], p: usize }
impl<'a> TwBuf<'a> {
    fn new(d: &'a [u8]) -> TwBuf<'a> { TwBuf { d, p: 0 } }
    fn err() -> TwTlsErr { tw_tls_err(TW_ALERT_DECODE_ERROR, "malformed handshake message") }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], TwTlsErr> {
        if self.p + n > self.d.len() { return Err(TwBuf::err()); }
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<usize, TwTlsErr> { Ok(self.bytes(1)?[0] as usize) }
    fn u16(&mut self) -> Result<usize, TwTlsErr> { let b = self.bytes(2)?; Ok(((b[0] as usize) << 8) | b[1] as usize) }
    fn u24(&mut self) -> Result<usize, TwTlsErr> { let b = self.bytes(3)?; Ok(((b[0] as usize) << 16) | ((b[1] as usize) << 8) | b[2] as usize) }
    fn vec8(&mut self) -> Result<&'a [u8], TwTlsErr> { let n = self.u8()?; self.bytes(n) }
    fn vec16(&mut self) -> Result<&'a [u8], TwTlsErr> { let n = self.u16()?; self.bytes(n) }
    fn vec24(&mut self) -> Result<&'a [u8], TwTlsErr> { let n = self.u24()?; self.bytes(n) }
    fn done(&self) -> bool { self.p == self.d.len() }
    fn end(&self) -> Result<(), TwTlsErr> { if self.done() { Ok(()) } else { Err(TwBuf::err()) } }
}

/// Extensions as (type, body), refusing duplicates.
fn tw_parse_exts(data: &[u8]) -> Result<Vec<(u16, Vec<u8>)>, TwTlsErr> {
    let mut b = TwBuf::new(data);
    let mut out: Vec<(u16, Vec<u8>)> = Vec::new();
    while !b.done() {
        let t = b.u16()? as u16;
        let v = b.vec16()?.to_vec();
        if out.iter().any(|(k, _)| *k == t) { return Err(tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "duplicate TLS extension")); }
        out.push((t, v));
    }
    Ok(out)
}
fn tw_find_ext(exts: &[(u16, Vec<u8>)], t: u16) -> Option<&[u8]> { exts.iter().find(|(k, _)| *k == t).map(|(_, v)| v.as_slice()) }

/// Key schedule secrets derived from the (EC)DHE shared secret.
fn tw_handshake_secret(alg: TwHashAlg, shared: &[u8]) -> Vec<u8> {
    let zeros = vec![0u8; alg.len()];
    let early = tw_hkdf_extract(alg, &[], &zeros);
    let derived = tw_expand_label(alg, &early, "derived", &tw_hash(alg, &[]), alg.len());
    tw_hkdf_extract(alg, &derived, shared)
}
fn tw_master_secret(alg: TwHashAlg, hs_secret: &[u8]) -> Vec<u8> {
    let derived = tw_expand_label(alg, hs_secret, "derived", &tw_hash(alg, &[]), alg.len());
    tw_hkdf_extract(alg, &derived, &vec![0u8; alg.len()])
}

/// The content a CertificateVerify signs (RFC 8446 section 4.4.3).
fn tw_cv_content(server: bool, transcript_hash: &[u8]) -> Vec<u8> {
    let mut m = vec![0x20u8; 64];
    m.extend_from_slice(if server { b"TLS 1.3, server CertificateVerify" } else { b"TLS 1.3, client CertificateVerify" });
    m.push(0);
    m.extend_from_slice(transcript_hash);
    m
}

fn tw_scheme_alg(scheme: u16) -> Option<TwSigAlg> {
    Some(match scheme {
        TW_SIG_ECDSA_P256 => TwSigAlg::Ecdsa(TwHashAlg::Sha256),
        TW_SIG_ECDSA_P384 => TwSigAlg::Ecdsa(TwHashAlg::Sha384),
        TW_SIG_PSS_256 => TwSigAlg::RsaPss(TwHashAlg::Sha256),
        TW_SIG_PSS_384 => TwSigAlg::RsaPss(TwHashAlg::Sha384),
        TW_SIG_PSS_512 => TwSigAlg::RsaPss(TwHashAlg::Sha512),
        _ => return None,
    })
}

/// Our key share for one group: (private, public).
fn tw_key_share(group: u16) -> (Vec<u8>, Vec<u8>) {
    if group == TW_GROUP_X25519 {
        let mut k = vec![0u8; 32];
        tw_fill_random(&mut k);
        let p = tw_x25519_base(&k).to_vec();
        (k, p)
    } else {
        let c = tw_p256();
        let d = c.random_scalar();
        let p = c.public_key(&d).unwrap_or_default();
        (d, p)
    }
}
fn tw_shared_secret(group: u16, private: &[u8], peer: &[u8]) -> Result<Vec<u8>, TwTlsErr> {
    if group == TW_GROUP_X25519 {
        if peer.len() != 32 { return Err(tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "bad X25519 key share")); }
        let s = tw_x25519(private, peer);
        if s.iter().all(|&b| b == 0) { return Err(tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "bad X25519 key share")); }
        Ok(s.to_vec())
    } else {
        tw_p256().ecdh(private, peer).ok_or_else(|| tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "bad secp256r1 key share"))
    }
}

// ===========================================================================
// 5. Client handshake
// ===========================================================================

/// TLS 1.3 handshake as a client over `sock`, verifying the certificate for `host`.
fn tw_tls_connect(sock: std::net::TcpStream, host: &str) -> Result<TwTls, String> {
    let anchors = tw_trust_anchors()?;
    tw_tls_connect_with(sock, host, anchors)
}
fn tw_tls_connect_with(sock: std::net::TcpStream, host: &str, anchors: &[TwCert]) -> Result<TwTls, String> {
    let mut tls = TwTls::new(sock);
    match tw_tls_client_handshake(&mut tls, host, anchors) {
        Ok(()) => Ok(tls),
        Err(e) => {
            if e.alert != TW_ALERT_CLOSE_NOTIFY { tls.send_alert(e.alert); }
            Err(e.message)
        }
    }
}

fn tw_tls_client_handshake(tls: &mut TwTls, host: &str, anchors: &[TwCert]) -> Result<(), TwTlsErr> {
    let (x_priv, x_pub) = tw_key_share(TW_GROUP_X25519);
    let (p_priv, p_pub) = tw_key_share(TW_GROUP_P256);
    let mut random = [0u8; 32];
    tw_fill_random(&mut random);
    let mut session_id = [0u8; 32];
    tw_fill_random(&mut session_id);

    let mut exts = Vec::new();
    let sni_host = host.trim_end_matches('.');
    if tw_host_ip(sni_host).is_none() {
        let mut sni = Vec::new();
        tw_put_u16(&mut sni, sni_host.len() + 3);
        sni.push(0);
        tw_put_u16(&mut sni, sni_host.len());
        sni.extend_from_slice(sni_host.as_bytes());
        tw_ext(&mut exts, 0, &sni);
    }
    tw_ext(&mut exts, 10, &[0, 4, 0x00, 0x1d, 0x00, 0x17]);
    let schemes = [TW_SIG_ECDSA_P256, TW_SIG_ECDSA_P384, TW_SIG_PSS_256, TW_SIG_PSS_384, TW_SIG_PSS_512, TW_SIG_PKCS1_256, TW_SIG_PKCS1_384, TW_SIG_PKCS1_512];
    let mut sa = Vec::new();
    tw_put_u16(&mut sa, schemes.len() * 2);
    for s in schemes { tw_put_u16(&mut sa, s as usize); }
    tw_ext(&mut exts, 13, &sa);
    tw_ext(&mut exts, 16, b"\x00\x09\x08http/1.1");
    tw_ext(&mut exts, 43, &[2, 3, 4]);
    let mut ks = Vec::new();
    tw_put_u16(&mut ks, 4 + x_pub.len() + 4 + p_pub.len());
    tw_put_u16(&mut ks, TW_GROUP_X25519 as usize); tw_put_u16(&mut ks, x_pub.len()); ks.extend_from_slice(&x_pub);
    tw_put_u16(&mut ks, TW_GROUP_P256 as usize); tw_put_u16(&mut ks, p_pub.len()); ks.extend_from_slice(&p_pub);
    tw_ext(&mut exts, 51, &ks);

    let mut body = vec![3, 3];
    body.extend_from_slice(&random);
    body.push(32);
    body.extend_from_slice(&session_id);
    body.extend_from_slice(&[0, 6, 0x13, 0x01, 0x13, 0x03, 0x13, 0x02, 1, 0]);
    tw_put_u16(&mut body, exts.len());
    body.extend_from_slice(&exts);
    tls.send_handshake(&tw_hs_msg(TW_HS_CLIENT_HELLO, &body))?;

    // ServerHello
    let sh = tls.expect_handshake(TW_HS_SERVER_HELLO, "ServerHello").map_err(|mut e| {
        if e.message.contains("alert") || e.message.contains("closed") { e.message.push_str(" in reply to the ClientHello; the server probably does not support TLS 1.3 (TLS 1.2 and older are not supported)"); }
        e
    })?;
    let mut b = TwBuf::new(&sh[4..]);
    let legacy = b.u16()?;
    let server_random = b.bytes(32)?;
    let echo = b.vec8()?;
    let suite = b.u16()? as u16;
    let compression = b.u8()?;
    let sexts = tw_parse_exts(b.vec16()?)?;
    b.end()?;
    let version = tw_find_ext(&sexts, 43);
    if legacy != 0x0303 || version != Some(&[3, 4][..]) {
        return Err(tw_tls_err(TW_ALERT_PROTOCOL_VERSION, "the server does not support TLS 1.3 (TLS 1.2 and older are not supported)"));
    }
    if server_random == TW_HRR_RANDOM { return Err(tw_tls_err(TW_ALERT_HANDSHAKE_FAILURE, "the server asked for a key exchange group that is not supported (HelloRetryRequest)")); }
    if echo != session_id || compression != 0 || ![TW_SUITE_AES128, TW_SUITE_AES256, TW_SUITE_CHACHA].contains(&suite) {
        return Err(tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "invalid ServerHello"));
    }
    let share = tw_find_ext(&sexts, 51).ok_or_else(|| tw_tls_err(TW_ALERT_MISSING_EXTENSION, "the server sent no key share (pre-shared keys are not supported)"))?;
    let mut sb = TwBuf::new(share);
    let group = sb.u16()? as u16;
    let peer = sb.vec16()?;
    sb.end()?;
    let shared = match group {
        TW_GROUP_X25519 => tw_shared_secret(group, &x_priv, peer)?,
        TW_GROUP_P256 => tw_shared_secret(group, &p_priv, peer)?,
        _ => return Err(tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "the server chose a key exchange group that was not offered")),
    };
    tls.suite = suite;
    if !tls.hs.is_empty() { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "handshake data after the ServerHello in the same record")); }
    tls.transcript.extend_from_slice(&sh);
    let alg = tls.hash();
    let hs_secret = tw_handshake_secret(alg, &shared);
    let th = tls.transcript_hash();
    let c_hs = tw_expand_label(alg, &hs_secret, "c hs traffic", &th, alg.len());
    let s_hs = tw_expand_label(alg, &hs_secret, "s hs traffic", &th, alg.len());
    tls.set_read_secret(s_hs.clone());

    // EncryptedExtensions, [CertificateRequest], Certificate, CertificateVerify, Finished
    let ee = tls.expect_handshake(TW_HS_ENCRYPTED_EXTENSIONS, "EncryptedExtensions")?;
    let mut eb = TwBuf::new(&ee[4..]);
    let eexts = tw_parse_exts(eb.vec16()?)?;
    eb.end()?;
    if let Some(alpn) = tw_find_ext(&eexts, 16) { if alpn != b"\x00\x09\x08http/1.1" { return Err(tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "the server chose an application protocol other than http/1.1")); } }
    tls.transcript.extend_from_slice(&ee);
    let mut msg = tls.read_handshake()?;
    let mut cert_requested = false;
    if msg[0] == TW_HS_CERTIFICATE_REQUEST {
        cert_requested = true;
        tls.transcript.extend_from_slice(&msg);
        msg = tls.read_handshake()?;
    }
    if msg[0] != TW_HS_CERTIFICATE { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "expected the server's Certificate")); }
    let mut cb = TwBuf::new(&msg[4..]);
    let context = cb.vec8()?;
    let list = cb.vec24()?;
    cb.end()?;
    if !context.is_empty() { return Err(tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "invalid Certificate")); }
    let mut lb = TwBuf::new(list);
    let mut chain: Vec<Vec<u8>> = Vec::new();
    while !lb.done() {
        chain.push(lb.vec24()?.to_vec());
        lb.vec16()?;
        if chain.len() > 16 { return Err(tw_tls_err(TW_ALERT_BAD_CERTIFICATE, "the certificate chain is too long")); }
    }
    let now = tw_now_ms();
    if let Err(e) = tw_verify_chain(&chain, host, anchors, now) {
        let alert = if e.contains("trusted") { TW_ALERT_UNKNOWN_CA } else { TW_ALERT_BAD_CERTIFICATE };
        return Err(tw_tls_err(alert, format!("certificate check failed: {}", e)));
    }
    let leaf = tw_parse_cert(&chain[0]).ok_or_else(|| tw_tls_err(TW_ALERT_BAD_CERTIFICATE, "the server certificate cannot be read"))?;
    tls.transcript.extend_from_slice(&msg);

    let cv = tls.expect_handshake(TW_HS_CERTIFICATE_VERIFY, "CertificateVerify")?;
    let mut vb = TwBuf::new(&cv[4..]);
    let scheme = vb.u16()? as u16;
    let sig = vb.vec16()?;
    vb.end()?;
    let sig_alg = tw_scheme_alg(scheme).ok_or_else(|| tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "the server used a signature scheme that was not offered"))?;
    let curve_ok = match (&leaf.key, scheme) {
        (TwPubKey::Ec { curve: TwCurveId::P256, .. }, TW_SIG_ECDSA_P256) | (TwPubKey::Ec { curve: TwCurveId::P384, .. }, TW_SIG_ECDSA_P384) => true,
        (TwPubKey::Rsa { .. }, TW_SIG_PSS_256 | TW_SIG_PSS_384 | TW_SIG_PSS_512) => true,
        _ => false,
    };
    if !curve_ok || !tw_verify_signature(&leaf.key, sig_alg, &tw_cv_content(true, &tls.transcript_hash()), sig) {
        return Err(tw_tls_err(TW_ALERT_DECRYPT_ERROR, "the server's handshake signature is invalid"));
    }
    tls.transcript.extend_from_slice(&cv);

    let fin = tls.expect_handshake(TW_HS_FINISHED, "Finished")?;
    let expected = tls.finished_data(&s_hs);
    if !tw_ct_eq(&fin[4..], &expected) { return Err(tw_tls_err(TW_ALERT_DECRYPT_ERROR, "the server's Finished message is wrong")); }
    tls.transcript.extend_from_slice(&fin);
    if !tls.hs.is_empty() { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "handshake data after the server's Finished")); }

    let master = tw_master_secret(alg, &hs_secret);
    let th = tls.transcript_hash();
    let c_ap = tw_expand_label(alg, &master, "c ap traffic", &th, alg.len());
    let s_ap = tw_expand_label(alg, &master, "s ap traffic", &th, alg.len());

    tls.send_raw(&[TW_CT_CCS, 3, 3, 0, 1, 1])?;
    tls.set_write_secret(c_hs.clone());
    if cert_requested { tls.send_handshake(&tw_hs_msg(TW_HS_CERTIFICATE, &[0, 0, 0, 0]))?; }
    let ours = tls.finished_data(&c_hs);
    tls.send_handshake(&tw_hs_msg(TW_HS_FINISHED, &ours))?;
    tls.set_write_secret(c_ap);
    tls.set_read_secret(s_ap);
    tls.handshake_done = true;
    Ok(())
}

// ===========================================================================
// 6. Server handshake and configuration
// ===========================================================================

#[derive(Clone)]
enum TwPrivateKey { Ec { curve: TwCurveId, d: Vec<u8> }, Rsa(TwRsaKey) }

/// Certificate chain (leaf first, DER) and private key for `serve`.
#[derive(Clone)]
pub struct TwTlsConfig { chain: Vec<Vec<u8>>, key: TwPrivateKey }

fn tw_parse_rsa_private(der: &[u8]) -> Option<TwRsaKey> {
    let mut d = TwDer::new(TwDer::new(der).expect(0x30)?);
    d.expect(0x02)?; // version
    let mut next = || tw_der_uint(d.expect(0x02)?);
    let n = next()?; let e = next()?; let _dd = next()?; let p = next()?; let q = next()?;
    let dp = next()?; let dq = next()?; let qinv = next()?;
    Some(TwRsaKey { n, e, p, q, dp, dq, qinv })
}
fn tw_parse_ec_private(der: &[u8], curve: Option<TwCurveId>) -> Option<TwPrivateKey> {
    let mut d = TwDer::new(TwDer::new(der).expect(0x30)?);
    d.expect(0x02)?;
    let key = d.expect(0x04)?.to_vec();
    let mut id = curve;
    if let Some(params) = d.optional(0xa0) {
        let oid = TwDer::new(params).expect(0x06)?;
        id = if oid == TW_OID_P256 { Some(TwCurveId::P256) } else if oid == TW_OID_P384 { Some(TwCurveId::P384) } else { None };
    }
    let id = id?;
    let size = tw_curve(id).size;
    if key.len() > size { return None; }
    let mut padded = vec![0u8; size - key.len()];
    padded.extend_from_slice(&key);
    Some(TwPrivateKey::Ec { curve: id, d: padded })
}
fn tw_parse_pkcs8(der: &[u8]) -> Option<TwPrivateKey> {
    let mut d = TwDer::new(TwDer::new(der).expect(0x30)?);
    d.expect(0x02)?;
    let mut alg = TwDer::new(d.expect(0x30)?);
    let oid = alg.expect(0x06)?;
    let key = d.expect(0x04)?;
    if oid == TW_OID_RSA { return tw_parse_rsa_private(key).map(TwPrivateKey::Rsa); }
    if oid == TW_OID_EC {
        let c = alg.expect(0x06)?;
        let id = if c == TW_OID_P256 { TwCurveId::P256 } else if c == TW_OID_P384 { TwCurveId::P384 } else { return None };
        return tw_parse_ec_private(key, Some(id));
    }
    None
}

/// Reads a PEM certificate chain and a PEM private key (PKCS #8, SEC 1 or PKCS #1; RSA or ECDSA P-256/P-384).
pub fn tw_tls_load_config(cert_path: &str, key_path: &str) -> Result<TwTlsConfig, String> {
    let cert_text = std::fs::read_to_string(cert_path).map_err(|e| format!("cannot read the certificate file {}: {}", cert_path, e))?;
    let key_text = std::fs::read_to_string(key_path).map_err(|e| format!("cannot read the key file {}: {}", key_path, e))?;
    let chain: Vec<Vec<u8>> = tw_pem_blocks(&cert_text).into_iter().filter(|(l, _)| l == "CERTIFICATE").map(|(_, d)| d).collect();
    if chain.is_empty() { return Err(format!("{} holds no PEM certificate", cert_path)); }
    let leaf = tw_parse_cert(&chain[0]).ok_or_else(|| format!("the first certificate in {} cannot be read", cert_path))?;
    let mut key = None;
    for (label, der) in tw_pem_blocks(&key_text) {
        key = match label.as_str() {
            "PRIVATE KEY" => tw_parse_pkcs8(&der),
            "EC PRIVATE KEY" => tw_parse_ec_private(&der, None),
            "RSA PRIVATE KEY" => tw_parse_rsa_private(&der).map(TwPrivateKey::Rsa),
            "ENCRYPTED PRIVATE KEY" => return Err(format!("{} is encrypted; store the key without a passphrase (openssl pkey -in {} -out plain.pem)", key_path, key_path)),
            _ => continue,
        };
        if key.is_some() { break; }
        return Err(format!("the key in {} is not supported (use RSA of 2048 bits or more, or ECDSA P-256/P-384)", key_path));
    }
    let key = key.ok_or_else(|| format!("{} holds no PEM private key", key_path))?;
    let matches = match (&key, &leaf.key) {
        (TwPrivateKey::Ec { curve, d }, TwPubKey::Ec { curve: c2, point }) => curve == c2 && tw_curve(*curve).public_key(d).as_deref() == Some(point.as_slice()),
        (TwPrivateKey::Rsa(k), TwPubKey::Rsa { n, e }) => tw_strip_zeros(&k.n) == tw_strip_zeros(n) && tw_strip_zeros(&k.e) == tw_strip_zeros(e) && tw_rsa_mod_bits(n) >= 2048,
        _ => false,
    };
    if !matches { return Err(format!("the private key in {} does not belong to the certificate in {}", key_path, cert_path)); }
    Ok(TwTlsConfig { chain, key })
}

/// TLS 1.3 handshake as a server.
fn tw_tls_accept(sock: std::net::TcpStream, config: &TwTlsConfig) -> Result<TwTls, String> {
    let mut tls = TwTls::new(sock);
    match tw_tls_server_handshake(&mut tls, config) {
        Ok(()) => Ok(tls),
        Err(e) => {
            if e.alert != TW_ALERT_CLOSE_NOTIFY { tls.send_alert(e.alert); }
            Err(e.message)
        }
    }
}

fn tw_tls_server_handshake(tls: &mut TwTls, config: &TwTlsConfig) -> Result<(), TwTlsErr> {
    let ch = tls.expect_handshake(TW_HS_CLIENT_HELLO, "ClientHello")?;
    if !tls.hs.is_empty() { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "handshake data after the ClientHello")); }
    let mut b = TwBuf::new(&ch[4..]);
    b.u16()?;
    b.bytes(32)?;
    let session_id = b.vec8()?.to_vec();
    let suites_raw = b.vec16()?;
    let compression = b.vec8()?;
    let exts = if b.done() { Vec::new() } else { tw_parse_exts(b.vec16()?)? };
    b.end()?;
    if session_id.len() > 32 || !compression.contains(&0) { return Err(tw_tls_err(TW_ALERT_ILLEGAL_PARAMETER, "invalid ClientHello")); }
    let versions = tw_find_ext(&exts, 43).unwrap_or(&[]);
    let offers_13 = versions.len() >= 3 && versions[1..].chunks(2).any(|v| v == [3, 4]);
    if !offers_13 { return Err(tw_tls_err(TW_ALERT_PROTOCOL_VERSION, "the client does not offer TLS 1.3")); }
    let suites: Vec<u16> = suites_raw.chunks(2).filter(|c| c.len() == 2).map(|c| ((c[0] as u16) << 8) | c[1] as u16).collect();
    let suite = [TW_SUITE_CHACHA, TW_SUITE_AES128, TW_SUITE_AES256].into_iter().find(|s| suites.contains(s))
        .ok_or_else(|| tw_tls_err(TW_ALERT_HANDSHAKE_FAILURE, "no common cipher suite"))?;
    // key share
    let ks = tw_find_ext(&exts, 51).ok_or_else(|| tw_tls_err(TW_ALERT_MISSING_EXTENSION, "the client sent no key share"))?;
    let mut kb = TwBuf::new(ks);
    let mut kl = TwBuf::new(kb.vec16()?);
    kb.end()?;
    let mut chosen: Option<(u16, Vec<u8>)> = None;
    while !kl.done() {
        let g = kl.u16()? as u16;
        let k = kl.vec16()?.to_vec();
        if chosen.is_none() && (g == TW_GROUP_X25519 || g == TW_GROUP_P256) { chosen = Some((g, k)); }
    }
    let (group, peer) = chosen.ok_or_else(|| tw_tls_err(TW_ALERT_HANDSHAKE_FAILURE, "the client offered no X25519 or secp256r1 key share"))?;
    // signature scheme for our key
    let sig_algs: Vec<u16> = match tw_find_ext(&exts, 13) {
        Some(s) => { let mut sb = TwBuf::new(s); sb.vec16()?.chunks(2).filter(|c| c.len() == 2).map(|c| ((c[0] as u16) << 8) | c[1] as u16).collect() }
        None => return Err(tw_tls_err(TW_ALERT_MISSING_EXTENSION, "the client sent no signature algorithms")),
    };
    let scheme = match &config.key {
        TwPrivateKey::Ec { curve: TwCurveId::P256, .. } => Some(TW_SIG_ECDSA_P256).filter(|s| sig_algs.contains(s)),
        TwPrivateKey::Ec { curve: TwCurveId::P384, .. } => Some(TW_SIG_ECDSA_P384).filter(|s| sig_algs.contains(s)),
        TwPrivateKey::Rsa(_) => [TW_SIG_PSS_256, TW_SIG_PSS_384, TW_SIG_PSS_512].into_iter().find(|s| sig_algs.contains(s)),
    }.ok_or_else(|| tw_tls_err(TW_ALERT_HANDSHAKE_FAILURE, "the client accepts no signature scheme for this server's key"))?;
    // ALPN: http/1.1 only
    let mut alpn_reply: Option<Vec<u8>> = None;
    if let Some(a) = tw_find_ext(&exts, 16) {
        let mut ab = TwBuf::new(a);
        let mut list = TwBuf::new(ab.vec16()?);
        let mut found = false;
        while !list.done() { if list.vec8()? == b"http/1.1" { found = true; } }
        if !found { return Err(tw_tls_err(TW_ALERT_NO_APPLICATION_PROTOCOL, "the client does not speak http/1.1")); }
        alpn_reply = Some(b"\x00\x09\x08http/1.1".to_vec());
    }

    let (private, public) = tw_key_share(group);
    let shared = tw_shared_secret(group, &private, &peer)?;
    tls.suite = suite;
    tls.transcript.extend_from_slice(&ch);
    let mut random = [0u8; 32];
    tw_fill_random(&mut random);
    let mut sexts = Vec::new();
    tw_ext(&mut sexts, 43, &[3, 4]);
    let mut share = Vec::new();
    tw_put_u16(&mut share, group as usize);
    tw_put_u16(&mut share, public.len());
    share.extend_from_slice(&public);
    tw_ext(&mut sexts, 51, &share);
    let mut body = vec![3, 3];
    body.extend_from_slice(&random);
    body.push(session_id.len() as u8);
    body.extend_from_slice(&session_id);
    tw_put_u16(&mut body, suite as usize);
    body.push(0);
    tw_put_u16(&mut body, sexts.len());
    body.extend_from_slice(&sexts);
    tls.send_handshake(&tw_hs_msg(TW_HS_SERVER_HELLO, &body))?;
    if !session_id.is_empty() { tls.send_raw(&[TW_CT_CCS, 3, 3, 0, 1, 1])?; }

    let alg = tls.hash();
    let hs_secret = tw_handshake_secret(alg, &shared);
    let th = tls.transcript_hash();
    let c_hs = tw_expand_label(alg, &hs_secret, "c hs traffic", &th, alg.len());
    let s_hs = tw_expand_label(alg, &hs_secret, "s hs traffic", &th, alg.len());
    tls.set_write_secret(s_hs.clone());
    tls.set_read_secret(c_hs.clone());

    let mut ee = Vec::new();
    if let Some(a) = &alpn_reply { tw_ext(&mut ee, 16, a); }
    let mut eeb = Vec::new();
    tw_put_u16(&mut eeb, ee.len());
    eeb.extend_from_slice(&ee);
    let mut flight = Vec::new();
    let ee_msg = tw_hs_msg(TW_HS_ENCRYPTED_EXTENSIONS, &eeb);
    tls.transcript.extend_from_slice(&ee_msg);
    flight.extend_from_slice(&ee_msg);
    let mut list = Vec::new();
    for c in &config.chain { tw_put_u24(&mut list, c.len()); list.extend_from_slice(c); tw_put_u16(&mut list, 0); }
    let mut cbody = vec![0u8];
    tw_put_u24(&mut cbody, list.len());
    cbody.extend_from_slice(&list);
    let cert_msg = tw_hs_msg(TW_HS_CERTIFICATE, &cbody);
    tls.transcript.extend_from_slice(&cert_msg);
    flight.extend_from_slice(&cert_msg);
    let content = tw_cv_content(true, &tls.transcript_hash());
    let signature = match &config.key {
        TwPrivateKey::Ec { curve, d } => {
            let h = if *curve == TwCurveId::P256 { TwHashAlg::Sha256 } else { TwHashAlg::Sha384 };
            let (r, s) = tw_curve(*curve).sign(d, &tw_hash(h, &content));
            tw_ecdsa_der_encode(&r, &s)
        }
        TwPrivateKey::Rsa(k) => {
            let h = match scheme { TW_SIG_PSS_384 => TwHashAlg::Sha384, TW_SIG_PSS_512 => TwHashAlg::Sha512, _ => TwHashAlg::Sha256 };
            tw_rsa_sign_pss(k, h, &content).ok_or_else(|| tw_tls_err(TW_ALERT_INTERNAL_ERROR, "RSA signing failed"))?
        }
    };
    let mut cv = Vec::new();
    tw_put_u16(&mut cv, scheme as usize);
    tw_put_u16(&mut cv, signature.len());
    cv.extend_from_slice(&signature);
    let cv_msg = tw_hs_msg(TW_HS_CERTIFICATE_VERIFY, &cv);
    tls.transcript.extend_from_slice(&cv_msg);
    flight.extend_from_slice(&cv_msg);
    let fin = tw_hs_msg(TW_HS_FINISHED, &tls.finished_data(&s_hs));
    tls.transcript.extend_from_slice(&fin);
    flight.extend_from_slice(&fin);
    tls.write_record(TW_CT_HANDSHAKE, &flight)?;

    let master = tw_master_secret(alg, &hs_secret);
    let th = tls.transcript_hash();
    let c_ap = tw_expand_label(alg, &master, "c ap traffic", &th, alg.len());
    let s_ap = tw_expand_label(alg, &master, "s ap traffic", &th, alg.len());
    tls.set_write_secret(s_ap);

    let cfin = tls.expect_handshake(TW_HS_FINISHED, "the client's Finished")?;
    let expected = tls.finished_data(&c_hs);
    if !tw_ct_eq(&cfin[4..], &expected) { return Err(tw_tls_err(TW_ALERT_DECRYPT_ERROR, "the client's Finished message is wrong")); }
    if !tls.hs.is_empty() { return Err(tw_tls_err(TW_ALERT_UNEXPECTED_MESSAGE, "handshake data after the client's Finished")); }
    tls.set_read_secret(c_ap);
    tls.handshake_done = true;
    Ok(())
}

// ===========================================================================
// 7. Streams for the HTTP code
// ===========================================================================

/// A shared handle to one TLS connection; the HTTP reader and writer each hold one.
#[derive(Clone)]
struct TwTlsStream(std::rc::Rc<std::cell::RefCell<TwTls>>);

impl TwTlsStream {
    fn new(tls: TwTls) -> TwTlsStream { TwTlsStream(std::rc::Rc::new(std::cell::RefCell::new(tls))) }
    /// Sends close_notify (once).
    fn close(&self) { self.0.borrow_mut().send_alert(TW_ALERT_CLOSE_NOTIFY); }
    fn tcp(&self) -> Option<std::net::TcpStream> { self.0.borrow().sock.try_clone().ok() }
}
impl std::io::Read for TwTlsStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.0.borrow_mut().read_app(buf) }
}
impl std::io::Write for TwTlsStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { self.0.borrow_mut().write_app(buf) }
    fn flush(&mut self) -> std::io::Result<()> { self.0.borrow_mut().sock.flush() }
}

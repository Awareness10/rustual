//! The Python semantics `textual.validation` builds on, so validators accept and describe values
//! exactly as Textual does: `float()` / `int()` parsing, `repr()` of floats and strings, and the
//! scheme / netloc split of `urllib.parse.urlparse`.

use super::tables::{DECIMAL_ZEROS, NFKC_URL_DELIMITERS, NON_PRINTABLE, SPACES};

// ---------------------------------------------------------------------------------- characters

/// The value of a decimal digit in any script (Python `unicodedata.decimal`).
fn decimal_value(c: char) -> Option<u8> {
    let cp = c as u32;
    let i = DECIMAL_ZEROS.partition_point(|&zero| zero <= cp);
    let offset = cp - *DECIMAL_ZEROS.get(i.checked_sub(1)?)?;
    (offset < 10).then_some(offset as u8)
}

/// Python `str.isspace()`.
pub(super) fn is_space(c: char) -> bool {
    SPACES.binary_search(&(c as u32)).is_ok()
}

/// Python `str.isprintable()` for one character.
fn is_printable(c: char) -> bool {
    let cp = c as u32;
    let i = NON_PRINTABLE.partition_point(|&(start, _)| start <= cp);
    i == 0 || NON_PRINTABLE[i - 1].1 < cp
}

/// CPython's `_PyUnicode_TransformDecimalAndSpaceToASCII`: ASCII is kept as is, other
/// whitespace becomes a space, decimal digits of any script become ASCII digits, and anything
/// else becomes `?` (invalid).
fn to_ascii_number(s: &str) -> String {
    s.chars()
        .map(|c| {
            if (c as u32) < 127 {
                c
            } else if is_space(c) {
                ' '
            } else if let Some(d) = decimal_value(c) {
                char::from(b'0' + d)
            } else {
                '?'
            }
        })
        .collect()
}

/// CPython's `Py_ISSPACE`: the whitespace `float()` / `int()` strip (ASCII only, after the
/// transformation above has turned other whitespace into spaces).
fn trim_ascii_space(s: &str) -> &str {
    s.trim_matches([' ', '\t', '\n', '\x0B', '\x0C', '\r'])
}

// ---------------------------------------------------------------------------------- numbers

/// CPython's `_Py_string_to_number_with_underscores`: underscores are only allowed between
/// digits. `None` if one is misplaced.
fn remove_underscores(s: &str) -> Option<String> {
    if !s.contains('_') {
        return Some(s.to_string());
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut prev = bytes[0];
    out.push(char::from(prev));
    for &b in &bytes[1..] {
        if b == b'_' {
            if !prev.is_ascii_digit() {
                return None;
            }
        } else {
            out.push(char::from(b));
            if prev == b'_' && !b.is_ascii_digit() {
                return None;
            }
        }
        prev = b;
    }
    (prev != b'_').then_some(out)
}

/// Whether `s` (ASCII, trimmed) is a finite decimal literal: `[sign] (digits [. [digits]] |
/// . digits) [(e|E) [sign] digits]`.
fn is_decimal_literal(s: &str) -> bool {
    let b = s.strip_prefix(['+', '-']).unwrap_or(s).as_bytes();
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(0);
    let mut i = int;
    let mut frac = 0;
    if b.get(i) == Some(&b'.') {
        frac = digits(i + 1);
        i += 1 + frac;
    }
    if int + frac == 0 {
        return false;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let exp = digits(i);
        if exp == 0 {
            return false;
        }
        i += exp;
    }
    i == b.len()
}

/// Python `float(s)`: `None` where Python raises `ValueError`. Infinities and NaN (`"inf"`,
/// `"nan"`, `"1e999"`) parse, as in Python.
pub(crate) fn py_float(s: &str) -> Option<f64> {
    let ascii = remove_underscores(&to_ascii_number(s))?;
    let t = trim_ascii_space(&ascii);
    let body = t.strip_prefix(['+', '-']).unwrap_or(t);
    let special = body.eq_ignore_ascii_case("inf")
        || body.eq_ignore_ascii_case("infinity")
        || body.eq_ignore_ascii_case("nan");
    if !special && !is_decimal_literal(t) {
        return None;
    }
    t.parse::<f64>().ok()
}

/// Python's default limit on the digits `int()` converts (`sys.get_int_max_str_digits()`).
const INT_MAX_STR_DIGITS: usize = 4300;

/// Whether Python `int(s)` (base 10) succeeds.
pub(crate) fn py_int_ok(s: &str) -> bool {
    let ascii = to_ascii_number(s);
    let t = trim_ascii_space(&ascii);
    let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
    !digits.is_empty()
        && !digits.starts_with('_')
        && !digits.ends_with('_')
        && !digits.contains("__")
        && digits.bytes().all(|b| b.is_ascii_digit() || b == b'_')
        && digits.bytes().filter(u8::is_ascii_digit).count() <= INT_MAX_STR_DIGITS
}

// ---------------------------------------------------------------------------------- repr

/// Python `repr(x)` for a float (`1.0`, `0.0001`, `1e-05`, `1e+16`, `inf`, `nan`).
pub(crate) fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    // Rust's `{:e}` gives the shortest round-trip digits, like Python's repr.
    let formatted = format!("{x:e}");
    let (mantissa, exp) = formatted.split_once('e').expect("exponent");
    let exp: i32 = exp.parse().expect("exponent");
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(m) => ("-", m),
        None => ("", mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    if (-4..16).contains(&exp) {
        let (int, frac) = if exp >= 0 {
            let int_len = exp as usize + 1;
            if digits.len() <= int_len {
                (format!("{digits:0<int_len$}"), "0".to_string())
            } else {
                (digits[..int_len].to_string(), digits[int_len..].to_string())
            }
        } else {
            let zeros = "0".repeat((-exp - 1) as usize);
            ("0".to_string(), format!("{zeros}{digits}"))
        };
        format!("{sign}{int}.{frac}")
    } else {
        let (first, rest) = digits.split_at(1);
        let dot = if rest.is_empty() { "" } else { "." };
        let exp_sign = if exp < 0 { '-' } else { '+' };
        format!("{sign}{first}{dot}{rest}e{exp_sign}{:02}", exp.abs())
    }
}

/// Python `repr(s)` for a string (`'abc'`, `"it's"`, `'\x1b'`).
pub(crate) fn str_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        let cp = c as u32;
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            _ if cp < 0x20 || cp == 0x7F => out.push_str(&format!("\\x{cp:02x}")),
            c if c.is_ascii() || is_printable(c) => out.push(c),
            _ if cp <= 0xFF => out.push_str(&format!("\\x{cp:02x}")),
            _ if cp <= 0xFFFF => out.push_str(&format!("\\u{cp:04x}")),
            _ => out.push_str(&format!("\\U{cp:08x}")),
        }
    }
    out.push(quote);
    out
}

// ---------------------------------------------------------------------------------- URLs

/// Whether `urllib.parse.urlparse(url)` succeeds with both a scheme and a netloc (what Textual's
/// `URL` validator checks). Ports `_urlsplit` as far as it determines those two.
pub(crate) fn url_has_scheme_and_netloc(url: &str) -> bool {
    let url: String = url
        .trim_start_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\r' | '\n'))
        .collect();
    let mut scheme = "";
    let mut rest = url.as_str();
    if let Some(i) = url.find(':') {
        let starts_alpha = url.starts_with(|c: char| c.is_ascii_alphabetic());
        let candidate = &url[..i];
        if i > 0
            && starts_alpha
            && candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            scheme = candidate;
            rest = &url[i + 1..];
        }
    }
    let Some(after) = rest.strip_prefix("//") else {
        return false;
    };
    let netloc = &after[..after.find(['/', '?', '#']).unwrap_or(after.len())];
    let (open, close) = (netloc.contains('['), netloc.contains(']'));
    if open != close || (open && !bracketed_netloc_ok(netloc)) || !netloc_nfkc_ok(netloc) {
        return false;
    }
    !scheme.is_empty() && !netloc.is_empty()
}

/// `urllib.parse._checknetloc`: no character may turn into a URL delimiter under NFKC.
fn netloc_nfkc_ok(netloc: &str) -> bool {
    netloc.is_ascii()
        || !netloc
            .chars()
            .any(|c| NFKC_URL_DELIMITERS.binary_search(&(c as u32)).is_ok())
}

/// `urllib.parse._check_bracketed_netloc`.
fn bracketed_netloc_ok(netloc: &str) -> bool {
    let host_and_port = netloc.rsplit_once('@').map_or(netloc, |(_, h)| h);
    let hostname = match host_and_port.split_once('[') {
        Some((before, bracketed)) => {
            if !before.is_empty() {
                return false;
            }
            let (host, port) = bracketed.split_once(']').unwrap_or((bracketed, ""));
            if !port.is_empty() && !port.starts_with(':') {
                return false;
            }
            host
        }
        None => host_and_port
            .split_once(':')
            .map_or(host_and_port, |(h, _)| h),
    };
    bracketed_host_ok(hostname)
}

/// `urllib.parse._check_bracketed_host`: an IPvFuture address or an IPv6 address.
fn bracketed_host_ok(hostname: &str) -> bool {
    if let Some(rest) = hostname.strip_prefix(['v', 'V']) {
        // \A[vV][a-fA-F0-9]+\..+\z
        let hex = rest.bytes().take_while(u8::is_ascii_hexdigit).count();
        let Some(tail) = rest[hex..].strip_prefix('.') else {
            return false;
        };
        return hex > 0 && !tail.is_empty() && !tail.contains('\n');
    }
    !ipv4_ok(hostname) && ipv6_ok(hostname)
}

/// Python `ipaddress.IPv4Address(s)` succeeds.
fn ipv4_ok(s: &str) -> bool {
    if s.is_empty() || s.contains('/') {
        return false;
    }
    let octets: Vec<&str> = s.split('.').collect();
    octets.len() == 4
        && octets.iter().all(|o| {
            !o.is_empty()
                && o.len() <= 3
                && o.bytes().all(|b| b.is_ascii_digit())
                && (*o == "0" || !o.starts_with('0'))
                && o.parse::<u16>().is_ok_and(|v| v <= 255)
        })
}

/// Python `ipaddress.IPv6Address(s)` succeeds (with an optional `%scope`).
fn ipv6_ok(s: &str) -> bool {
    if s.contains('/') {
        return false;
    }
    let addr = match s.split_once('%') {
        Some((addr, scope)) if !scope.is_empty() && !scope.contains('%') => addr,
        Some(_) => return false,
        None => s,
    };
    if addr.is_empty() || addr.chars().count() > 45 {
        return false;
    }
    const HEXTETS: usize = 8;
    let mut parts: Vec<String> = addr.splitn(HEXTETS + 2, ':').map(String::from).collect();
    if parts.len() < 3 {
        return false;
    }
    if let Some(v4) = parts.pop_if(|p| p.contains('.')) {
        if !ipv4_ok(&v4) {
            return false;
        }
        // The value doesn't matter, only that it becomes two valid hextets.
        parts.push("0".into());
        parts.push("0".into());
    }
    if parts.len() > HEXTETS + 1 {
        return false;
    }
    let n = parts.len();
    let mut skip = None;
    for (i, part) in parts.iter().enumerate().take(n - 1).skip(1) {
        if part.is_empty() {
            if skip.is_some() {
                return false;
            }
            skip = Some(i);
        }
    }
    let (hi, lo) = match skip {
        Some(skip) => {
            let (mut hi, mut lo) = (skip, n - skip - 1);
            if parts[0].is_empty() {
                hi -= 1;
                if hi > 0 {
                    return false;
                }
            }
            if parts[n - 1].is_empty() {
                lo -= 1;
                if lo > 0 {
                    return false;
                }
            }
            if HEXTETS < hi + lo + 1 {
                return false;
            }
            (hi, lo)
        }
        None => {
            if n != HEXTETS || parts[0].is_empty() || parts[n - 1].is_empty() {
                return false;
            }
            (n, 0)
        }
    };
    let hextet =
        |p: &String| !p.is_empty() && p.len() <= 4 && p.bytes().all(|b| b.is_ascii_hexdigit());
    parts[..hi].iter().all(hextet) && parts[n - lo..].iter().all(hextet)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_digits_of_any_script() {
        assert_eq!(decimal_value('7'), Some(7));
        assert_eq!(decimal_value('\u{0663}'), Some(3)); // ARABIC-INDIC DIGIT THREE
        assert_eq!(decimal_value('\u{1D7D8}'), Some(0)); // MATHEMATICAL DOUBLE-STRUCK ZERO
        assert_eq!(decimal_value('a'), None);
        assert_eq!(decimal_value('\u{00B2}'), None); // superscript two: a digit, not decimal
        assert_eq!(decimal_value('\u{2167}'), None); // ROMAN NUMERAL EIGHT
    }

    #[test]
    fn spaces_and_printables() {
        for c in [' ', '\t', '\u{1c}', '\u{85}', '\u{a0}', '\u{3000}'] {
            assert!(is_space(c), "{c:?}");
        }
        for c in ['a', '\u{200b}', '\u{feff}'] {
            assert!(!is_space(c), "{c:?}");
        }
        assert!(is_printable('a') && is_printable('😀') && is_printable(' '));
        assert!(
            !is_printable('\u{a0}') && !is_printable('\u{200b}') && !is_printable('\u{10ffff}')
        );
    }

    #[test]
    fn python_float() {
        for (s, v) in [
            ("1", 1.0),
            (" -1.5 ", -1.5),
            ("1_000.000_1", 1000.0001),
            (".5", 0.5),
            ("5.", 5.0),
            ("1e3", 1000.0),
            ("+1E-3", 0.001),
            ("\u{0661}\u{0662}", 12.0),
            ("\u{3000}7\u{2003}", 7.0),
            ("42\x0B\x0C", 42.0),
        ] {
            assert_eq!(py_float(s), Some(v), "{s:?}");
        }
        assert!(py_float("inf").unwrap().is_infinite());
        assert!(py_float("-Infinity").unwrap().is_infinite());
        assert!(py_float("nAn").unwrap().is_nan());
        assert!(py_float("1e999").unwrap().is_infinite());
        for s in [
            "",
            " ",
            ".",
            "e3",
            "1e",
            "1e+",
            "1_",
            "_1",
            "1__0",
            "1_.5",
            "1._5",
            "1e_3",
            "0x10",
            "1 2",
            "++1",
            "in_f",
            "1,5",
            "١.٢x",
            "infinit",
            "nan(1)",
            "1\u{0}",
            "\x1c42",
            "42\u{1f}",
            "\u{1c}42\u{3000}",
        ] {
            assert_eq!(py_float(s), None, "{s:?}");
        }
    }

    #[test]
    fn python_int() {
        for s in [
            "0",
            "007",
            "-12",
            " +1_000 ",
            "\u{0967}\u{0968}",
            "\u{2003}5\n",
        ] {
            assert!(py_int_ok(s), "{s:?}");
        }
        for s in [
            "", "-", "1.0", "1e3", "_1", "1_", "1__0", "+_1", "- 1", "0x1", "١x",
        ] {
            assert!(!py_int_ok(s), "{s:?}");
        }
        assert!(py_int_ok(&"9".repeat(4300)));
        assert!(!py_int_ok(&"9".repeat(4301)));
    }

    #[test]
    fn python_float_repr() {
        for (x, s) in [
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (1e-4, "0.0001"),
            (1e-5, "1e-05"),
            (0.1, "0.1"),
            (1.5, "1.5"),
            (-0.0, "-0.0"),
            (0.0, "0.0"),
            (123456789012345678.0, "1.2345678901234568e+17"),
            (2.5e-7, "2.5e-07"),
            (1e100, "1e+100"),
            (5.0, "5.0"),
            (-12.25, "-12.25"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
        ] {
            assert_eq!(float_repr(x), s);
        }
    }

    #[test]
    fn python_str_repr() {
        assert_eq!(str_repr("ab"), "'ab'");
        assert_eq!(str_repr("a'b"), "\"a'b\"");
        assert_eq!(str_repr("a\"b'"), "'a\"b\\''");
        assert_eq!(str_repr("\\d+"), "'\\\\d+'");
        assert_eq!(
            str_repr("\x1b\x7f\u{a0}\u{200b}😀\t\n\r"),
            "'\\x1b\\x7f\\xa0\\u200b😀\\t\\n\\r'"
        );
        assert_eq!(str_repr("\u{10ffff}"), "'\\U0010ffff'");
    }

    #[test]
    fn urls() {
        for u in [
            "http://example.com",
            "https://user:pw@example.com:80/p?q#f",
            "  ftp://x",
            "h\tt\ntp://x",
            "git+ssh://host",
            "http://[::1]:8080",
            "http://[fe80::1%25eth0]",
            "http://[v1.fe]",
            "http://[::ffff:1.2.3.4]",
            "http://exa\u{e9}mple.com",
            "x://@",
        ] {
            assert!(url_has_scheme_and_netloc(u), "{u:?}");
        }
        for u in [
            "",
            "example.com",
            "http:/x",
            "//example.com",
            "1http://x",
            "ht_tp://x",
            "http://",
            "http:///path",
            "http://[::1",
            "http://::1]",
            "http://[1.2.3.4]",
            "http://[v.1]",
            "http://[vg.1]",
            "http://[::1]x",
            "http://a[::1]",
            "http://[::1%]",
            "http://[gggg::1]",
            "http://ex\u{2100}mple.com",
            "http://ex\u{ff0f}mple.com",
        ] {
            assert!(!url_has_scheme_and_netloc(u), "{u:?}");
        }
    }

    #[test]
    fn ip_addresses() {
        for s in ["1.2.3.4", "0.0.0.0", "255.255.255.255"] {
            assert!(ipv4_ok(s), "{s}");
        }
        for s in [
            "1.2.3",
            "1.2.3.4.5",
            "01.2.3.4",
            "256.1.1.1",
            "1.2.3.a",
            "1..3.4",
            "1.2.3.4/8",
            "١.2.3.4",
        ] {
            assert!(!ipv4_ok(s), "{s}");
        }
        for s in [
            "::",
            "::1",
            "1::",
            "1:2:3:4:5:6:7:8",
            "1::8",
            "1:2:3:4:5:6::",
            "::2:3:4:5:6:7:8",
            "fe80::1%eth0",
            "::ffff:1.2.3.4",
            "1:2:3:4:5:6:1.2.3.4",
            "ABCD::ef01",
        ] {
            assert!(ipv6_ok(s), "{s}");
        }
        for s in [
            "",
            ":",
            "1:2",
            ":::",
            "1:::2",
            "1::2::3",
            ":1::2",
            "1::2:",
            "1:2:3:4:5:6:7:8:9",
            "1:2:3:4:5:6:7",
            "1:2:3:4:5:6:7::8",
            "12345::",
            "g::",
            "::1%",
            "::1%a%b",
            "1:2:3:4:5:6:7:1.2.3.4",
            "::1.2.3",
            "::1/64",
            &"0:".repeat(23),
        ] {
            assert!(!ipv6_ok(s), "{s}");
        }
    }
}

//! Compile a Python `re` pattern for `re.fullmatch` with the `regex` crate.
//!
//! Translates the Python syntax whose meaning differs (or that `regex` rejects although Python
//! accepts it), so common patterns behave as in Textual:
//!
//! - `\Z` / `\z` (end of string) → `\z`; `(?#comment)` is removed,
//! - octal escapes (`\0`, `\012`, `\101`) and `\b` inside a class (backspace) → `\x{..}`,
//! - a `[` inside a class and a `{` that doesn't start a quantifier are literal; `{,n}` → `{0,n}`,
//! - `&` / `~` inside a class are literal (not `regex` set operations),
//! - with `re.ASCII`, `\d \w \s` (and negations) become ASCII classes and `\b \B` ASCII boundaries.
//!
//! Python features `regex` lacks (backreferences, look-around, conditionals, atomic groups,
//! possessive quantifiers) make compilation fail rather than silently match something else.

use super::RegexFlags;

/// Compile `pattern` so that `is_match` is Python's `re.fullmatch(pattern, value, flags)`.
pub(super) fn compile_fullmatch(
    pattern: &str,
    flags: RegexFlags,
) -> Result<regex::Regex, regex::Error> {
    let (pattern, flags) = leading_flags(pattern, flags)?;
    let known = RegexFlags::IGNORECASE
        | RegexFlags::MULTILINE
        | RegexFlags::DOTALL
        | RegexFlags::UNICODE
        | RegexFlags::VERBOSE
        | RegexFlags::ASCII;
    if flags.bits() & !known.bits() != 0 {
        return Err(regex::Error::Syntax(format!(
            "unsupported regex flags: {}",
            flags.bits()
        )));
    }
    if flags.contains(RegexFlags::ASCII | RegexFlags::UNICODE) {
        return Err(regex::Error::Syntax(
            "ASCII and UNICODE flags are incompatible".into(),
        ));
    }
    let verbose = flags.contains(RegexFlags::VERBOSE);
    let body = translate(pattern, flags.contains(RegexFlags::ASCII), verbose)?;
    // In verbose mode a trailing `# comment` would swallow the closing group: end its line.
    let newline = if verbose { "\n" } else { "" };
    regex::RegexBuilder::new(&format!("\\A(?:{body}{newline})\\z"))
        .case_insensitive(flags.contains(RegexFlags::IGNORECASE))
        .multi_line(flags.contains(RegexFlags::MULTILINE))
        .dot_matches_new_line(flags.contains(RegexFlags::DOTALL))
        .ignore_whitespace(verbose)
        .build()
}

/// Python's global inline flags at the start of the pattern (`(?x)`, `(?ai)`): move them into
/// `flags`, as Python applies them to the whole pattern.
fn leading_flags(
    mut pattern: &str,
    mut flags: RegexFlags,
) -> Result<(&str, RegexFlags), regex::Error> {
    while let Some(rest) = pattern.strip_prefix("(?") {
        let len = rest.chars().take_while(|c| c.is_ascii_alphabetic()).count();
        if len == 0 || rest.as_bytes().get(len) != Some(&b')') {
            break;
        }
        for c in rest[..len].chars() {
            flags |= match c {
                'i' => RegexFlags::IGNORECASE,
                'm' => RegexFlags::MULTILINE,
                's' => RegexFlags::DOTALL,
                'u' => RegexFlags::UNICODE,
                'x' => RegexFlags::VERBOSE,
                'a' => RegexFlags::ASCII,
                _ => {
                    return Err(regex::Error::Syntax(format!(
                        "unsupported inline flag {c:?}"
                    )))
                }
            };
        }
        pattern = &rest[len + 1..];
    }
    Ok((pattern, flags))
}

const ASCII_DIGIT: &str = "0-9";
const ASCII_WORD: &str = "0-9A-Za-z_";
const ASCII_SPACE: &str = r"\t\n\x0B\x0C\r ";

/// The class for a shorthand escape under `re.ASCII` (`None` if `c` isn't one).
fn ascii_class(c: char) -> Option<String> {
    let (set, negated) = match c {
        'd' => (ASCII_DIGIT, false),
        'D' => (ASCII_DIGIT, true),
        'w' => (ASCII_WORD, false),
        'W' => (ASCII_WORD, true),
        's' => (ASCII_SPACE, false),
        'S' => (ASCII_SPACE, true),
        _ => return None,
    };
    Some(format!("[{}{set}]", if negated { "^" } else { "" }))
}

/// Whether `chars[i..]` is a Python repetition quantifier body after `{`: `m}`, `m,}`, `,n}`,
/// `m,n}` (digits only). Returns its length including the `}`, and whether the minimum is
/// missing.
fn quantifier(chars: &[char], i: usize) -> Option<(usize, bool)> {
    let digits = |from: usize| {
        chars[from..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count()
    };
    let min = digits(i);
    let mut j = i + min;
    if chars.get(j) == Some(&',') {
        j += 1 + digits(j + 1);
    } else if min == 0 {
        return None;
    }
    (chars.get(j) == Some(&'}')).then_some((j + 1 - i, min == 0))
}

/// An octal escape at `chars[i..]` (after the backslash): `\0`, `\0d`, `\0dd`, or three octal
/// digits. Returns its length and the code point.
fn octal(chars: &[char], i: usize) -> Option<(usize, u32)> {
    let is_octal = |k: usize| chars.get(k).is_some_and(|c| ('0'..='7').contains(c));
    if chars[i] == '0' {
        let len = 1 + (1..3).take_while(|k| is_octal(i + k)).count();
        let value = chars[i..i + len]
            .iter()
            .fold(0, |v, c| v * 8 + c.to_digit(8).unwrap());
        return Some((len, value));
    }
    if is_octal(i) && is_octal(i + 1) && is_octal(i + 2) {
        let value = chars[i..i + 3]
            .iter()
            .fold(0, |v, c| v * 8 + c.to_digit(8).unwrap());
        return (value <= 0o377).then_some((3, value));
    }
    None
}

fn translate(pattern: &str, ascii: bool, verbose: bool) -> Result<String, regex::Error> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut i = 0;
    // Inside a character class: the index of its first member (where `]` is literal).
    let mut class_start: Option<usize> = None;
    // The previous token was a repetition (`*`, `+`, `?`, `{m,n}`): a `+` now is possessive.
    let mut after_repeat = false;
    while i < chars.len() {
        let c = chars[i];
        let repeat = std::mem::take(&mut after_repeat);
        if c == '\\' {
            let Some(&next) = chars.get(i + 1) else {
                return Err(regex::Error::Syntax("bad escape (end of pattern)".into()));
            };
            if let Some((len, value)) = octal(&chars, i + 1) {
                out.push_str(&format!("\\x{{{value:x}}}"));
                i += 1 + len;
                continue;
            }
            match next {
                'Z' | 'z' if class_start.is_none() => out.push_str("\\z"),
                'b' if class_start.is_some() => out.push_str("\\x{8}"),
                'b' | 'B' if ascii => out.push_str(&format!("(?-u:\\{next})")),
                _ => match ascii_class(next).filter(|_| ascii) {
                    Some(class) => out.push_str(&class),
                    None => {
                        out.push('\\');
                        out.push(next);
                    }
                },
            }
            i += 2;
            continue;
        }
        if let Some(start) = class_start {
            match c {
                ']' if i > start => class_start = None,
                '[' => {
                    out.push_str("\\[");
                    i += 1;
                    continue;
                }
                '&' | '~' => out.push('\\'),
                // Python's verbose mode keeps whitespace and `#` in a class; regex's doesn't.
                '#' if verbose => out.push('\\'),
                c if verbose && c.is_whitespace() => {
                    out.push_str(&format!("\\x{{{:x}}}", c as u32));
                    i += 1;
                    continue;
                }
                _ => {}
            }
            out.push(c);
            i += 1;
            continue;
        }
        match c {
            '[' => {
                out.push('[');
                i += 1;
                if chars.get(i) == Some(&'^') {
                    out.push('^');
                    i += 1;
                }
                class_start = Some(i);
                continue;
            }
            '(' if chars.get(i + 1) == Some(&'?') && chars.get(i + 2) == Some(&'#') => {
                // A comment group runs to the first `)`.
                match chars[i..].iter().position(|&c| c == ')') {
                    Some(end) => i += end + 1,
                    None => {
                        return Err(regex::Error::Syntax(
                            "missing ), unterminated comment".into(),
                        ))
                    }
                }
                continue;
            }
            '{' => match quantifier(&chars, i + 1) {
                Some((len, no_min)) => {
                    out.push('{');
                    if no_min {
                        out.push('0');
                    }
                    out.extend(&chars[i + 1..i + 1 + len]);
                    i += 1 + len;
                    after_repeat = true;
                    continue;
                }
                None => out.push_str("\\{"),
            },
            '#' if verbose => {
                // A comment to the end of the line: copy it as is (regex ignores it too).
                let end = chars[i..]
                    .iter()
                    .position(|&c| c == '\n')
                    .map_or(chars.len(), |e| i + e);
                out.extend(&chars[i..end]);
                i = end;
                continue;
            }
            '+' if repeat => {
                return Err(regex::Error::Syntax(
                    "possessive quantifiers are not supported".into(),
                ))
            }
            '*' | '+' | '?' => {
                out.push(c);
                after_repeat = true;
            }
            _ => out.push(c),
        }
        i += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fullmatch(pattern: &str, flags: RegexFlags, value: &str) -> bool {
        compile_fullmatch(pattern, flags).unwrap().is_match(value)
    }

    #[test]
    fn translations() {
        let t = |p: &str| translate(p, false, false).unwrap();
        assert_eq!(t(r"a\Z"), r"a\z");
        assert_eq!(t(r"a(?#comment)b"), "ab");
        assert_eq!(t(r"\0\012\101\8"), r"\x{0}\x{a}\x{41}\8");
        assert_eq!(t(r"[[a]"), r"[\[a]");
        assert_eq!(t(r"[]a]"), r"[]a]");
        assert_eq!(t(r"[^]a]"), r"[^]a]");
        assert_eq!(t(r"[a&&b~~c]"), r"[a\&\&b\~\~c]");
        assert_eq!(t(r"[\b]"), r"[\x{8}]");
        assert_eq!(t(r"a{2}b{2,}c{,3}d{1,2}"), r"a{2}b{2,}c{0,3}d{1,2}");
        assert_eq!(t(r"a{x}{"), r"a\{x}\{");
        assert_eq!(t(r"\[x\]"), r"\[x\]");
        assert_eq!(
            translate(r"\d\W\b", true, false).unwrap(),
            r"[0-9][^0-9A-Za-z_](?-u:\b)"
        );
        assert!(translate("a\\", false, false).is_err());
        assert!(translate("(?#x", false, false).is_err());
    }

    #[test]
    fn python_semantics() {
        let none = RegexFlags::NONE;
        assert!(fullmatch(r"\d+", none, "123"));
        assert!(!fullmatch(r"\d+", none, "123a"));
        assert!(!fullmatch(r"\d+", none, "a123"));
        assert!(fullmatch(r"\d+", none, "١٢٣")); // Unicode digits, as in Python
        assert!(!fullmatch(r"\d+", RegexFlags::ASCII, "١٢٣"));
        assert!(fullmatch(r"\w+", RegexFlags::ASCII, "a_1"));
        assert!(!fullmatch(r"\w+", RegexFlags::ASCII, "é"));
        assert!(fullmatch(r"\W", RegexFlags::ASCII, "é"));
        assert!(fullmatch("a|b", none, "b")); // the alternation is grouped
        assert!(!fullmatch("a|b", none, "ab"));
        assert!(fullmatch("abc", RegexFlags::IGNORECASE, "ABC"));
        assert!(fullmatch("a.b", RegexFlags::DOTALL, "a\nb"));
        assert!(!fullmatch("a.b", none, "a\nb"));
        assert!(fullmatch("a$\n^b", RegexFlags::MULTILINE, "a\nb"));
        assert!(fullmatch("a b # comment", RegexFlags::VERBOSE, "ab"));
        assert!(fullmatch("x{2}", none, "xx"));
        assert!(fullmatch("a+?b*?", none, "aab")); // lazy quantifiers
        assert!(fullmatch(r"\++", none, "++"));
        assert!(fullmatch("[+]+", none, "++"));
        assert!(fullmatch("x{", none, "x{"));
        assert!(fullmatch("[[]", none, "["));
        assert!(fullmatch(r"\0", none, "\0"));
        assert!(fullmatch(r"a\Z", none, "a"));
        assert!(fullmatch("(?i)abc", none, "ABC"));
        assert!(fullmatch("(?x) a b # c", none, "ab"));
        assert!(fullmatch("(?a)\\w", none, "a") && !fullmatch("(?a)\\w", none, "é"));
        assert!(fullmatch("[ #]+", RegexFlags::VERBOSE, " # "));
        assert!(fullmatch("", none, ""));
    }

    #[test]
    fn unsupported() {
        for p in [
            r"(a)\1", "(?=a)a", "(?<=a)b", "(?P=x)", "a++", "a*+", "a?+", "a{2}+", "(", "[a",
        ] {
            assert!(compile_fullmatch(p, RegexFlags::NONE).is_err(), "{p}");
        }
        assert!(compile_fullmatch("a", RegexFlags::ASCII | RegexFlags::UNICODE).is_err());
        assert!(compile_fullmatch("a", RegexFlags(4)).is_err()); // re.LOCALE
        assert!(compile_fullmatch("(?L)a", RegexFlags::NONE).is_err());
    }
}

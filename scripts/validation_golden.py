"""Run Textual's validators on a corpus of values and write the results as goldens.

    uv run --python 3.14.7 --no-project --with textual==8.2.8 python scripts/validation_golden.py

Use the Python that generated `crates/rustual/src/validation/tables.rs` (`urllib.parse` and the
Unicode data change between Python releases).

Writes (both from the same data, so rustual and rustual-py are held to the same answers):

- crates/rustual/tests/golden/validation.rs: `golden_validator(spec)` (each spec built with the
  Rust API) and `GOLDEN`: `(spec, value, [(failure kind, failure value, description)])`,
- crates/rustual-py/tests/data/validation_golden.json: the same, with each spec's Python
  constructor.

The corpus mixes hand-picked edge cases with seeded random strings that exercise the `float()` /
`int()` grammar and `urllib.parse`, so it is the same on every run.
"""

from __future__ import annotations

import json
import random
import re
from pathlib import Path

import sys

import textual
from textual.validation import URL, Integer, Length, Number, Regex

ROOT = Path(__file__).resolve().parent.parent

# (spec id, Python constructor, Rust constructor)
NUMBER_SPECS = [
    ("number", "Number()", "Number::new()"),
    ("number_min0", "Number(minimum=0)", "Number::new().minimum(0)"),
    ("number_max10", "Number(maximum=10)", "Number::new().maximum(10)"),
    ("number_range_float", "Number(minimum=-1.5, maximum=2.5)", "Number::new().minimum(-1.5).maximum(2.5)"),
    ("number_range_mixed", "Number(minimum=0, maximum=1.0)", "Number::new().minimum(0).maximum(1.0)"),
    ("number_big", "Number(minimum=1e20, maximum=1e-05)", "Number::new().minimum(1e20).maximum(1e-5)"),
    ("number_desc", "Number(maximum=5, failure_description='Too big!')",
     "Number::new().maximum(5).failure_description(\"Too big!\")"),
    ("integer", "Integer()", "Integer::new()"),
    ("integer_range", "Integer(minimum=0, maximum=100)", "Integer::new().minimum(0).maximum(100)"),
    ("integer_max", "Integer(maximum=-5)", "Integer::new().maximum(-5)"),
    ("integer_min_float", "Integer(minimum=0.5)", "Integer::new().minimum(0.5)"),
    ("integer_huge", "Integer(minimum=-9007199254740993, maximum=9007199254740993)",
     "Integer::new().minimum(-9007199254740993i64).maximum(9007199254740993i64)"),
    ("integer_desc", "Integer(minimum=1, failure_description='Positive integers only')",
     "Integer::new().minimum(1).failure_description(\"Positive integers only\")"),
]
LENGTH_SPECS = [
    ("length", "Length()", "Length::new()"),
    ("length_min2", "Length(minimum=2)", "Length::new().minimum(2)"),
    ("length_max5", "Length(maximum=5)", "Length::new().maximum(5)"),
    ("length_2_5", "Length(minimum=2, maximum=5)", "Length::new().minimum(2).maximum(5)"),
    ("length_0_0", "Length(minimum=0, maximum=0)", "Length::new().minimum(0).maximum(0)"),
    ("length_desc", "Length(minimum=1, failure_description='Required')",
     "Length::new().minimum(1).failure_description(\"Required\")"),
]
URL_SPECS = [
    ("url", "URL()", "Url::new()"),
    ("url_desc", "URL(failure_description='Bad link')", "Url::new().failure_description(\"Bad link\")"),
]
# (spec id, pattern, Python flags expression, Rust flags expression, values)
REGEX_SPECS = [
    ("re_digits", r"\d+", "0", "RegexFlags::NONE", ["", "1", "123", "12a", "a12", "١٢٣", "1\n", " 1", "1 "]),
    ("re_alt", "cat|dog", "0", "RegexFlags::NONE", ["cat", "dog", "catdog", "ca", "", "Cat"]),
    ("re_ignorecase", "[a-z]+", "re.IGNORECASE", "RegexFlags::IGNORECASE", ["abc", "ABC", "aBc", "a1", "", "é"]),
    ("re_ascii", r"\w+", "re.ASCII", "RegexFlags::ASCII", ["abc_1", "é", "a é", "", "Ω"]),
    ("re_unicode_w", r"\w+", "0", "RegexFlags::NONE", ["abc_1", "é", "a é", "", "Ω", "٣"]),
    ("re_dotall", "a.b", "re.DOTALL", "RegexFlags::DOTALL", ["a.b", "axb", "a\nb", "ab"]),
    ("re_dot", "a.b", "0", "RegexFlags::NONE", ["a.b", "axb", "a\nb", "ab"]),
    ("re_multiline", "^a$\n^b$", "re.MULTILINE", "RegexFlags::MULTILINE", ["a\nb", "ab", "a\n\nb"]),
    ("re_verbose", r"\d{3} - \d{4}  # phone", "re.VERBOSE", "RegexFlags::VERBOSE",
     ["123-4567", "123 - 4567", "1234567", ""]),
    ("re_combined", "a.c", "re.IGNORECASE | re.DOTALL", "RegexFlags::IGNORECASE | RegexFlags::DOTALL",
     ["ABC", "a\nC", "ac"]),
    ("re_inline", "(?i)hello", "0", "RegexFlags::NONE", ["hello", "HeLLo", "hell"]),
    ("re_end_z", r"ab\Z", "0", "RegexFlags::NONE", ["ab", "ab\n", "abc"]),
    ("re_literal_brace", "a{x}", "0", "RegexFlags::NONE", ["a{x}", "ax", "a"]),
    ("re_quant", "x{2,3}y{,2}", "0", "RegexFlags::NONE", ["xx", "xxx", "xxxx", "xxyy", "xxyyy", "x"]),
    ("re_class_bracket", "[[a]+", "0", "RegexFlags::NONE", ["[a[", "a", "]", ""]),
    ("re_octal", r"\101\0", "0", "RegexFlags::NONE", ["A\0", "A", "101"]),
    ("re_comment", "a(?#note)b", "0", "RegexFlags::NONE", ["ab", "a(?#note)b"]),
    ("re_email", r"[^@\s]+@[^@\s]+\.[a-z]{2,}", "0", "RegexFlags::NONE",
     ["a@b.co", "a@b.c", "a b@c.de", "@b.co", "a@b@c.de"]),
    ("re_quotes", "it's \"x\"", "0", "RegexFlags::NONE", ["it's \"x\"", "x"]),
    ("re_escapes", "\t\x1b\\\\", "0", "RegexFlags::NONE", ["\t\x1b\\", "x"]),
    ("re_empty", "", "0", "RegexFlags::NONE", ["", "a"]),
    ("re_desc", "[0-9a-f]+", "0", "RegexFlags::NONE", ["beef", "xyz"]),
]

NUMBERS = [
    "", " ", "0", "-0", "+0", "1", "-1", "+1", "007", "1.", ".5", "-.5", "5.", ".", "-", "+", "e", "1e", "1e5",
    "1E5", "1e+5", "1e-5", "1e+", "e5", "1.5e3", "-1.5E-3", "1_000", "1__000", "_1", "1_", "1_.5", "1._5",
    "1.5_5", "1e1_0", "1e_10", "1_e10", "0x10", "0b1", "0o7", "1j", "inf", "-inf", "Infinity", "+INF", "nan",
    "NaN", "-nan", "infinity!", "1e308", "1e309", "-1e309", "1e-400", "9" * 400, "1" + "0" * 20,
    "123456789012345678901234567890", " 42 ", "\t42\n", "\u3000" + "42", "42\u2003", "\x1c42\x1f", "4 2",
    "\u0661\u0662", "\u0967.\u0968", "\U0001D7D9", "١x", "\u00b2", "\u2167", "1,5", "1,000", "$1", "5%", "--1",
    "+-1", "1e5.5", "0.0", "-0.0", "10", "10.0", "10.000001", "9.999999", "100", "101", "100.0", "-5", "-6",
    "-4.9", "0.5", "0.49", "1.5", "2.5", "2.5000001", "-1.5", "-1.50001", "1e20", "99999999999999999999",
    "100000000000000000001", "1e-05", "0.00001", "0.000009", "9007199254740993", "9007199254740994",
    "-9007199254740993", "-9007199254740995", "9007199254740992.5", "\x00", "1\x00", "one", "1 000", "１２",
]
LENGTHS = ["", "a", "ab", "abc", "abcd", "abcde", "abcdef", "  ", "é", "e\u0301", "😀😀", "日本語", "\n\n\n", "a" * 100]
URLS = [
    "", "http://example.com", "https://example.com/path?q=1#f", "example.com", "www.example.com", "http:/x",
    "http:x", "//example.com", "http://", "http:///p", "ftp://files.example.com", "mailto:a@b.c", "file:///etc",
    "file://host/etc", "HTTP://EXAMPLE.COM", "  http://x", "http://x  ", "\x00http://x", "h\tt\ntp://x",
    "http://user:pw@host:8080", "http://[::1]", "http://[::1]:80", "http://[::1", "http://::1]", "http://[1.2.3.4]",
    "http://[v1.fe]", "http://[V9.a:b]", "http://[v.1]", "http://[vg.1]", "http://[::1]x", "http://a[::1]",
    "http://[fe80::1%25eth0]", "http://[::1%]", "http://[gggg::1]", "http://[::ffff:1.2.3.4]",
    "http://[1:2:3:4:5:6:7:8]", "http://[1:2:3:4:5:6:7:8:9]", "http://u@[::1]", "http://[::1]@host",
    "http://ex\u2100mple.com", "http://ex\uff0fmple.com", "http://ex\u00e9mple.com", "http://\u65e5\u672c.jp",
    "1http://x", "ht_tp://x", "a+b-c.d://x", "x://@", "x://:", "x://?", "x://#", "x:/", ":", "://x", "http//x",
    "http:\\\\x", "javascript:alert(1)", "http://exa mple.com", "http://example.com:notaport",
]

NUM_ALPHABET = list("0123456789.eE+-_ ") + ["٣", "\u3000", "x", "inf", "nan", "\t"]
URL_TOKENS = [
    "http", "h", ":", "//", "/", "[", "]", "::1", "::", "v1.", "@", "%", "x", "1.2.3.4", "?", "#", " ", "\t",
    "\n", "é", "\u2100", "ff", ".", "-", "+", "0", "1", "fe80", "%25", "v", "\x00",
]


def fuzz(rng: random.Random, alphabet: list[str], count: int, max_len: int) -> list[str]:
    return ["".join(rng.choice(alphabet) for _ in range(rng.randint(1, max_len))) for _ in range(count)]


def build() -> tuple[list[tuple[str, str, str]], list[tuple[str, str, list[tuple[str, str | None, str | None]]]]]:
    rng = random.Random(8_2_8)
    specs: list[tuple[str, str, str]] = []
    cases = []

    def run(spec_id: str, py: str, rust: str, values: list[str]) -> None:
        specs.append((spec_id, py, rust))
        validator = eval(py, {"Number": Number, "Integer": Integer, "Length": Length, "URL": URL,
                              "Regex": Regex, "re": re})
        seen = set()
        for value in values:
            if value in seen:
                continue
            seen.add(value)
            result = validator.validate(value)
            failures = [(type(f).__qualname__, f.value, f.description) for f in result.failures]
            cases.append((spec_id, value, failures))

    number_fuzz = fuzz(rng, NUM_ALPHABET, 1500, 6)
    for spec_id, py, rust in NUMBER_SPECS:
        values = NUMBERS + (number_fuzz if spec_id in ("number", "integer") else [])
        run(spec_id, py, rust, values)
    for spec_id, py, rust in LENGTH_SPECS:
        run(spec_id, py, rust, LENGTHS)
    url_fuzz = fuzz(rng, URL_TOKENS, 2000, 7)
    for spec_id, py, rust in URL_SPECS:
        run(spec_id, py, rust, URLS + (url_fuzz if spec_id == "url" else []))
    for spec_id, pattern, py_flags, rust_flags, values in REGEX_SPECS:
        py = f"Regex({pattern!r}, flags={py_flags})"
        rust = f"Regex::with_flags({rust_str(pattern)}, {rust_flags}).unwrap()"
        if spec_id == "re_desc":
            py = f"Regex({pattern!r}, failure_description='Hex only')"
            rust = f"Regex::new({rust_str(pattern)}).unwrap().failure_description(\"Hex only\")"
        run(spec_id, py, rust, values)
    return specs, cases


def rust_str(s: str | None) -> str:
    if s is None:
        return "None"
    out = []
    for c in s:
        if c == "\\":
            out.append("\\\\")
        elif c == '"':
            out.append('\\"')
        elif " " <= c <= "~":
            out.append(c)
        else:
            out.append(f"\\u{{{ord(c):x}}}")
    return '"' + "".join(out) + '"'


def rust_opt(s: str | None) -> str:
    return "None" if s is None else f"Some({rust_str(s)})"


def main() -> None:
    specs, cases = build()
    header = (
        f"Generated by scripts/validation_golden.py from Textual {textual.__version__} on Python "
        f"{sys.version.split()[0]} — do not edit by hand."
    )

    rust = [f"// {header}", "", "/// The validator of a golden spec, built with the Rust API.",
            "fn golden_validator(spec: &str) -> Box<dyn Validator> {", "    match spec {"]
    for spec_id, _, ctor in specs:
        rust.append(f"        {rust_str(spec_id)} => Box::new({ctor}),")
    rust += ["        other => panic!(\"unknown golden spec {other}\"),", "    }", "}", "",
             "/// A golden failure: (failure kind, failure value, description).",
             "type GoldenFailure = (&'static str, Option<&'static str>, Option<&'static str>);", "",
             "/// (spec, value, failures) as Textual's validators report them.",
             "const GOLDEN: &[(&str, &str, &[GoldenFailure])] = &["]
    for spec_id, value, failures in cases:
        fs = ", ".join(f"({rust_str(k)}, {rust_opt(v)}, {rust_opt(d)})" for k, v, d in failures)
        rust.append(f"    ({rust_str(spec_id)}, {rust_str(value)}, &[{fs}]),")
    rust.append("];")
    (ROOT / "crates/rustual/tests/golden/validation.rs").write_text("\n".join(rust) + "\n")

    data = {
        "generated": header,
        # `URL` uses the interpreter's urllib.parse, which changes between Python releases.
        "python": sys.version.split()[0],
        "specs": {spec_id: py for spec_id, py, _ in specs},
        "cases": [[spec_id, value, failures] for spec_id, value, failures in cases],
    }
    out = ROOT / "crates/rustual-py/tests/data/validation_golden.json"
    out.parent.mkdir(exist_ok=True)
    out.write_text(json.dumps(data, ensure_ascii=True, indent=0) + "\n")
    print(f"{len(specs)} specs, {len(cases)} cases")


if __name__ == "__main__":
    main()

#![allow(clippy::expect_used, clippy::panic)]

use crate::tex::{esc, fmt_int_sep, fmt_sep};

#[test]
fn esc_escapes_specials_without_double_escaping() {
    assert_eq!(esc("A & B"), r"A \& B");
    assert_eq!(esc("100%"), r"100\%");
    assert_eq!(esc("a_b#c$d~e"), r"a\_b\#c\$d\~{}e");
    assert_eq!(esc(r"a\b"), r"a\textbackslash{}b");
}

#[test]
fn fmt_sep_matches_python_comma_format() {
    assert_eq!(fmt_sep(555.12, 2), "555.12");
    assert_eq!(fmt_sep(1850.4, 0), "1,850");
    assert_eq!(fmt_sep(13340.92, 0), "13,341");
    assert_eq!(fmt_sep(2570.0, 0), "2,570");
    assert_eq!(fmt_sep(40.0, 0), "40");
    assert_eq!(fmt_sep(1234567.891, 2), "1,234,567.89");
    assert_eq!(fmt_sep(0.3, 2), "0.30");
}

#[test]
fn fmt_int_sep_groups_thousands() {
    assert_eq!(fmt_int_sep(700), "700");
    assert_eq!(fmt_int_sep(4800), "4,800");
    assert_eq!(fmt_int_sep(17400), "17,400");
    assert_eq!(fmt_int_sep(1234567), "1,234,567");
}

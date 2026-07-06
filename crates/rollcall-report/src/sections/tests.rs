#![allow(clippy::expect_used, clippy::panic)]

use crate::sections::capitalize;

#[test]
fn capitalize_matches_python_str_capitalize() {
    assert_eq!(capitalize("store"), "Store");
    assert_eq!(capitalize("EDGE"), "Edge");
    assert_eq!(capitalize(""), "");
    assert_eq!(capitalize("a"), "A");
}

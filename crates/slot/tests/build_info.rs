use slot::build_info::Build;

fn sample() -> Build {
    Build {
        version: "0.1.0",
        hash: "9e11a10",
        dirty: false,
        date: "2026-08-12",
    }
}

/// The serial is upper case hex.
///
/// ★ 2026-10-29（六六）：这条原先还附带一句「它是条形码的载荷，Code 39 会拒绝字母表外的字符」。
/// 条形码随牌子上的字一起删了，那个约束也就不存在了 —— 上界只剩下"它是个可大写化的短哈希"。
/// 但**大写这件事本身仍然要求**：`serial()` 是给外面看的名字，大小写不稳定的话，
/// 同一版构建会有两种写法。
#[test]
fn the_serial_is_upper_case_hex() {
    assert_eq!(sample().serial(), "9E11A10");
    assert!(sample().serial().chars().all(|c| c.is_ascii_hexdigit()));
}

/// The dirty marker is `1` or `0` and nothing else, which is what the boxed digit used to show.
#[test]
fn the_boxed_digit_says_whether_the_tree_was_dirty() {
    let dirty = Build {
        dirty: true,
        ..sample()
    };
    assert_eq!(sample().dirty_digit(), '0');
    assert_eq!(dirty.dirty_digit(), '1');
}

/// Every probe fails soft. The device build runs in a container as root against a bind mount
/// owned by another uid, which is the case git refuses outright, and a frontend that would
/// not build because it could not name itself would be a poor trade.
#[test]
fn a_build_with_no_git_still_has_a_serial() {
    let unknown = Build {
        hash: "unknown",
        date: "unknown",
        ..sample()
    };
    // The fallback name is not empty and survives the same upper-casing as a real hash, so a
    // build that could not read git still names itself instead of reporting nothing.
    assert!(!unknown.serial().is_empty());
    assert_eq!(unknown.serial(), unknown.serial().to_uppercase());
}

/// Whatever this build's environment gave it, it is populated rather than empty.
#[test]
fn the_current_build_is_populated() {
    let b = Build::current();
    assert!(!b.version.is_empty());
    assert!(!b.hash.is_empty());
    assert!(!b.date.is_empty());
}

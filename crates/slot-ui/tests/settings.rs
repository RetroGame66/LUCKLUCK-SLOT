//! The settings rows are a list whose **visual order is `Row::ALL`**, and everything that
//! touches them — the double-column grid position (`row_at`), the navigation
//! (`up`/`down`/`left`/`right`), the face lookup in `app.rs::draw_settings` (`settings_rows`
//! is built in `ALL` order and looked up by `index`) — goes through `Row::index()`.
//!
//! ⚠️ **2026-10-28 事故**：`index()` 一度是 `self as usize`（枚举判别值），而 `Row::ALL` 的
//! 顺序在行序重排后与枚举定义顺序脱钩（枚举仍 `FastForward=0, Rewind=1, DateTime=2, …`，
//! `ALL` 却已排成 `DateTime, Refresh, Sound, Sleep, FastForward, …`）。判别值 ≠ `ALL` 下标
//! ⇒ 「日期与时间」整格消失（`FastForward` 用判别值 0 取到 DateTime 的 1 元素值列表，而
//! 它自己的当前档位是 2，越界返回 `None`，连盘子都不画）+ 方向键导航乱套。
//! 第一轮 `ALL` 恰好按判别值顺序排，掩盖了这个错位；第二轮起重排后暴露。
//!
//! 这三条测试就是为此而钉的：`index()` 必须等于它在 `ALL` 里的位置（不是判别值），
//! 否则 `draw_settings` 的错位会静默重现。

use slot_ui::SettingsRow;

/// Faces are baked in `ALL` order and looked up by `index`. Reordering `ALL` without fixing
/// `index` would silently put the wrong row's value list on the wrong cell (the 10-28 bug).
#[test]
fn rows_are_indexed_in_all_order() {
    for (i, row) in SettingsRow::ALL.iter().enumerate() {
        assert_eq!(row.index(), i, "{row:?} is out of order in ALL");
    }
}

/// The double-column grid is derived from the `ALL` position (`col = i % COLS`,
/// `row = i / COLS`), and navigation wraps within it. This is the behaviour the 10-28 bug
/// broke: navigation used the discriminant while the grid used the `ALL` position.
#[test]
fn navigation_keeps_the_column_and_wraps() {
    // ALL = DateTime, Refresh, Sound, Sleep, FastForward, Rewind, InsertResume, EjectSave,
    //       Boot, Wifi
    use slot_ui::SettingsRow::*;

    // Down from the top-left lands on the same column, one grid row down.
    assert_eq!(DateTime.down(), Sound);
    // Down from the top-right lands on the same column, one grid row down.
    assert_eq!(Refresh.down(), Sleep);
    // Up from the top row wraps to the bottom row, same column.
    assert_eq!(DateTime.up(), Boot);
    assert_eq!(Refresh.up(), Wifi);

    // Left/right cross columns, no wrap.
    assert_eq!(DateTime.right(), Refresh);
    assert_eq!(Refresh.left(), DateTime);
    assert_eq!(DateTime.left(), DateTime);
    assert_eq!(Refresh.right(), Refresh);

    // A vertical sweep keeps the column, so each column visits its own five rows. Sweep both
    // columns and the union covers every row exactly once.
    let mut seen: Vec<SettingsRow> = Vec::new();
    for start in [DateTime, Refresh] {
        let mut cur = start;
        for _ in 0..SettingsRow::ALL.len() {
            if !seen.contains(&cur) {
                seen.push(cur);
            }
            cur = cur.down();
        }
    }
    assert_eq!(seen.len(), SettingsRow::ALL.len());
    for r in SettingsRow::ALL {
        assert!(seen.contains(&r), "{r:?} missed by a vertical sweep");
    }
}

/// The face lookup in `app.rs::draw_settings` is `settings_rows.get(r.index())`, and the
/// array is built in `ALL` order. `index()` returning a discriminant is exactly the bug; this
/// pins the contract the caller relies on.
#[test]
fn index_never_exceeds_all() {
    for row in SettingsRow::ALL {
        assert!(row.index() < SettingsRow::ALL.len());
    }
}

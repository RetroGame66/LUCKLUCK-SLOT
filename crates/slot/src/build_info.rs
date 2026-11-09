//! What this binary is: version, git hash, dirty flag, build date.
//!
//! ★ 2026-10-29（六六）：这些字段**不再进关于页的牌子** —— 牌子上的字全删了，包括那枚
//! 编码 `serial()` 的条形码。本模块留着是因为它是"这个二进制是什么"的唯一记录点
//! （`--version` 一类的东西可以从这里取），只是不再有调用者把它印到贴纸上。

/// Everything known about the build. `'static` throughout because all of it is baked in by
/// `build.rs` at compile time.
#[derive(Copy, Clone, Debug)]
pub struct Build {
    pub version: &'static str,
    pub hash: &'static str,
    pub dirty: bool,
    pub date: &'static str,
}

impl Build {
    pub fn current() -> Build {
        Build {
            version: env!("CARGO_PKG_VERSION"),
            hash: env!("SLOT_GIT_HASH"),
            dirty: env!("SLOT_GIT_DIRTY") == "1",
            date: env!("SLOT_BUILD_DATE"),
        }
    }

    /// The short hash, upper case. **Nothing draws this any more** (it used to be the barcode's
    /// payload); kept because it names the build and callers outside the label may want it.
    pub fn serial(&self) -> String {
        self.hash.to_uppercase()
    }

    /// `1` for a build from a modified tree, `0` otherwise. Same story as `serial`: the boxed
    /// digit that showed it is gone with the rest of the label's type.
    pub fn dirty_digit(&self) -> char {
        match self.dirty {
            true => '1',
            false => '0',
        }
    }
}

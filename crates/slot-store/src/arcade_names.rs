//! 街机 romset 的中文名 —— 一张内置的对照表。
//!
//! 街机的 romset 名是硬件目录里的编号（`mslug`、`kof97`、`s1945`），不是人能读的名字，
//! 更不是中文名。这个模块给它一个中文名：**内置一份 9083 条的对照表**
//! （`assets/arcade_names.tsv`，生成脚本 `_tools/gen_arcade_names.py`，来源与处理见该脚本的
//! 文档），查不到就照旧显示 romset 名。
//!
//! # 为什么是内置而不是卡上外置
//!
//! 六六原话：「街机的中文名能否内置转换表？」这份表跟着二进制走，一张新卡插进来就有人名，
//! 不需要谁先去拷一个文件。卡上仍留一个**同格式的覆盖文件**（`System/arcade_names.tsv`），
//! 有就用它整份替换内置表 —— 见 [`init`]。这跟 `theme.txt` / `display.txt` / `labels.txt`
//! 是同一个思路：内置的是默认值，卡上的才是决定权。
//!
//! # 查法
//!
//! 表按 romset 的字节序排好，`binary_search_by` 一次约 14 次比较。
//! **第一次查表时才切分**（`OnceLock`），所以一张纯卡带的卡一次也不付这笔钱。
//! 切出来的 `Vec<(&str, &str)>` 是 9083 × 32 字节 ≈ 290 KB 常驻，只在街机货架上才会生成。
//!
//! # 只有街机走这里
//!
//! [`shown`] 是唯一的投递口，它对 GBA / GB / GBC / SFC / FC 一个字都不动 —— 那些卡带的文件名
//! 本来就是人写的名字，没有「英文编号」这回事。**别在别的机种上顺手也查一遍**：`sf2` 可能
//! 是某张 GBA 卡的 stem，把它显示成《街头霸王 II》是错的。

use std::path::Path;
use std::sync::OnceLock;

/// 内置表：`romset<TAB>中文名`，每行一条，LF 行尾，UTF-8。
const EMBEDDED: &str = include_str!("../assets/arcade_names.tsv");

/// 卡上的覆盖文件。有就整份替换内置表，没有就用内置表。
pub const OVERRIDE_FILE: &str = "System/arcade_names.tsv";

/// 切成对的表，按 key 排序。
type Table = Vec<(&'static str, &'static str)>;

static EMBEDDED_TABLE: OnceLock<Table> = OnceLock::new();

/// 卡上的覆盖表。`None` = 这张卡没有覆盖文件（**读过了**，不是还没读）。
///
/// 用一个 `OnceLock` 装着 `Option`，是为了区分「还没问过」和「问过了，没有」——
/// 每帧都去 stat 一次卡上那个文件是没有意义的。
static OVERRIDE: OnceLock<Option<Vec<(String, String)>>> = OnceLock::new();

/// 解析一份表：每行 `romset<TAB>名字`，`#` 开头的行是注释，空行跳过。
///
/// 返回的 `Vec` **按 key 的字节序排好**，这是查表的前提；输入不排序也能用，代价是启动时
/// 排一次（9083 条 ≈ 1 ms，只在开机发生一次）。
fn parse(text: &'static str) -> Table {
    let mut out: Table = Vec::with_capacity(9200);
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((romset, name)) = line.split_once('\t') else {
            continue;
        };
        let (romset, name) = (romset.trim(), name.trim());
        if romset.is_empty() || name.is_empty() {
            continue;
        }
        out.push((romset, name));
    }
    out.sort_unstable_by(|a, b| a.0.cmp(b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

fn embedded() -> &'static Table {
    EMBEDDED_TABLE.get_or_init(|| parse(EMBEDDED))
}

/// 读卡上的覆盖表。开机调一次（`App::boot`，和 `init_label_config` 挨着），
/// 之后 [`lookup`] 只读结果。
///
/// 文件不在、读不出、一条有效行都没有 ⇒ 保持内置表。**静默**：一张没有这个文件的卡是正常的，
/// 不是错误。
pub fn init(root: &Path) {
    let path = root.join(OVERRIDE_FILE);
    let table = std::fs::read_to_string(&path).ok().and_then(|text| {
        let mut rows: Vec<(String, String)> = Vec::new();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((romset, name)) = line.split_once('\t') else {
                continue;
            };
            let (romset, name) = (romset.trim().to_ascii_lowercase(), name.trim());
            if romset.is_empty() || name.is_empty() {
                continue;
            }
            rows.push((romset, name.to_string()));
        }
        if rows.is_empty() {
            return None;
        }
        rows.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        rows.dedup_by(|a, b| a.0 == b.0);
        Some(rows)
    });
    // 已经初始化过就不再动：`init` 只在开机和换卡时各一次，重复调用是调用方的错，
    // 但一次 `set` 失败也不该让开机崩掉 —— 于是忽略返回值。
    let _ = OVERRIDE.set(table);
}

/// 记下「这张卡没有覆盖文件」这件事。给 `init` 之外的开机路径用（测试里也用它复位）。
#[cfg(test)]
pub fn init_with_none() {
    let _ = OVERRIDE.set(None);
}

/// 一个 romset 的中文名。卡上有覆盖表就只查覆盖表，**不回退到内置表** ——
/// 整份替换的意思就是整份，混着用会出现「一半是旧译名一半是新译名」。
pub fn lookup(stem: &str) -> Option<&str> {
    let key = stem.to_ascii_lowercase();
    match OVERRIDE.get() {
        Some(Some(rows)) => rows
            .binary_search_by(|probe| probe.0.as_str().cmp(key.as_str()))
            .ok()
            .map(|i| rows[i].1.as_str()),
        // 覆盖表初始化过、但卡上没有 ⇒ 内置表。
        Some(None) | None => embedded()
            .binary_search_by(|probe| probe.0.cmp(key.as_str()))
            .ok()
            .map(|i| embedded()[i].1),
    }
}

/// 货架上这张卡该显示什么名字。**唯一的投递口**。
///
/// 街机（NeoGeo / 街机）先查中文表，查不到就回落到文件名清洗的结果；
/// 其余五个机种直接就是文件名清洗的结果，`lookup` 一次都不发生。
pub fn shown(system: crate::System, stem: &str) -> String {
    if matches!(system, crate::System::Neogeo | crate::System::Arcade) {
        if let Some(name) = lookup(stem) {
            return name.to_string();
        }
    }
    crate::clean_label(stem)
}

/// 表里有多少条 —— 开机日志一行，给「这张表是不是真的进来了」一个数字。
pub fn len() -> usize {
    match OVERRIDE.get() {
        Some(Some(rows)) => rows.len(),
        _ => embedded().len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 内置表本身：条目数、排序、以及几条必须查得到的。
    ///
    /// 排序是**功能**不是整洁：`binary_search_by` 在没排序的表上静默返回 `Err`，
    /// 表现就是「所有街机都显示 romset 名」，没有任何别的症状。
    #[test]
    fn the_embedded_table_is_sorted_and_answers() {
        let t = embedded();
        assert!(t.len() > 8000, "表太小了：{}", t.len());
        for w in t.windows(2) {
            assert!(w[0].0 < w[1].0, "表没排序，位置在 {:?}", w[0].0);
        }
        // 每个 key 都是小写 —— `lookup` 会把查询词转小写，表里大写就永远查不到。
        assert!(t.iter().all(|(k, _)| *k == k.to_ascii_lowercase()));
        // 每个名字都含汉字；纯英文的条目在生成时就被跳过了。
        let han = |s: &str| s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
        assert!(t.iter().all(|(_, v)| han(v)), "有条目没有汉字");
    }

    #[test]
    fn a_romset_resolves_to_its_chinese_name() {
        // 查表走覆盖表那条分支会先被 `init` 影响，所以这里直接问内置表。
        let t = embedded();
        let find = |k: &str| {
            t.binary_search_by(|p| p.0.cmp(k))
                .ok()
                .map(|i| t[i].1)
                .unwrap_or("")
        };
        assert_eq!(find("kof97"), "拳皇 '97");
        assert_eq!(find("dino"), "恐龙新世纪");
        assert_eq!(find("ddonpach"), "怒首领蜂");
        assert_eq!(find("nope-not-a-romset"), "");
    }

    /// 括号剥掉了：原表里 `恐龙新世纪 (世界版 930201)` 的括号是 dump 区号，不是名字。
    #[test]
    fn dump_tags_are_gone_from_the_names() {
        for (_, name) in embedded() {
            assert!(!name.contains('('), "还有括号：{name}");
            assert!(!name.contains('（'), "还有全角括号：{name}");
            assert!(!name.contains('['), "还有方括号：{name}");
        }
    }
}
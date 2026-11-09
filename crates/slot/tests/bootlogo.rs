//! 开机画面那一行的后端：卡上 `Bootlogos/` 的图，和"能不能写进分区"这件事。
//!
//! **这一份不碰分区**（宿主机上没有 `boot-resource`）：能在这里跑的是"卡上有什么、
//! 记了哪一张、什么样的 bmp 才算数"，以及**没写成的时候什么都没被改动** ——
//! 后者恰恰是这一屏唯一有风险的地方，所以它是这里的重点。
//! 真机上挂分区、覆盖、重启看效果，是设备上的活，不是测试能替的。

mod common;

use slot::bootlogo::{self, BYTES, Failure};
use std::path::Path;

/// 一张头合法的 bmp。**只造头**：像素全是 0，够判宽高、位深、压缩方式。
fn bmp(width: i32, height: i32, bpp: u16) -> Vec<u8> {
    let mut b = vec![0u8; BYTES];
    b[0..2].copy_from_slice(b"BM");
    b[2..6].copy_from_slice(&(BYTES as u32).to_le_bytes());
    b[10..14].copy_from_slice(&54u32.to_le_bytes());
    b[14..18].copy_from_slice(&40u32.to_le_bytes());
    b[18..22].copy_from_slice(&width.to_le_bytes());
    b[22..26].copy_from_slice(&height.to_le_bytes());
    b[26..28].copy_from_slice(&1u16.to_le_bytes());
    b[28..30].copy_from_slice(&bpp.to_le_bytes());
    b[30..34].copy_from_slice(&0u32.to_le_bytes());
    b
}

fn boot_dir(root: &Path) -> std::path::PathBuf {
    let dir = root.join(bootlogo::DIR);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 这两个用例会**真的去写分区**，所以只在没有 boot-resource 的机器上跑（也就是宿主机）。
/// 在设备上一路跑过去，等于把测试用的假图写进机器 —— 那是刷砖的走法，不是测试的走法。
fn on_a_device() -> bool {
    std::path::Path::new(bootlogo::PARTITION).exists()
}

/// 目录是用户的，里面会有别的东西，也会有 Mac 拷过来的影子文件。
#[test]
fn only_bmps_are_listed() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let dir = boot_dir(d.path());
    std::fs::write(dir.join("notes.txt"), b"hello").unwrap();
    std::fs::write(dir.join("cover.png"), b"not a bmp").unwrap();
    std::fs::write(dir.join("._psyduck.bmp"), b"sidecar").unwrap();
    assert!(bootlogo::list(d.path()).is_empty(), "a non bmp was listed");

    std::fs::write(dir.join("b.bmp"), bmp(720, 480, 24)).unwrap();
    std::fs::write(dir.join("a.BMP"), bmp(720, 480, 24)).unwrap();
    let names: Vec<String> = bootlogo::list(d.path())
        .iter()
        .map(|e| e.name.clone())
        .collect();
    // 排序按名字，不是按目录给回来的顺序 —— 同一张卡每次开机必须是同一列。
    assert_eq!(names, ["a.BMP", "b.bmp"], "got {names:?}");
}

/// 一列全是 bmp，后缀只是噪音。
#[test]
fn the_label_drops_the_extension() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let dir = boot_dir(d.path());
    std::fs::write(dir.join("sunset.bmp"), bmp(720, 480, 24)).unwrap();
    let logos = bootlogo::list(d.path());
    assert_eq!(logos[0].label(), "sunset");
    // 名字就是写进 `System/bootlogo.txt` 的那个（带扩展名），别把显示用的名字存进去。
    assert_eq!(logos[0].name, "sunset.bmp");
}

/// 卡上记的是**名字**：往目录里加一张图，编号就会错位，名字不会。
#[test]
fn the_card_remembers_which_picture_is_up() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let dir = boot_dir(d.path());
    for n in ["a.bmp", "b.bmp", "c.bmp"] {
        std::fs::write(dir.join(n), bmp(720, 480, 24)).unwrap();
    }
    let logos = bootlogo::list(d.path());
    assert!(bootlogo::current(d.path()).is_none(), "a blank card claimed a choice");

    bootlogo::save(d.path(), &logos[2].name);
    assert_eq!(bootlogo::current(d.path()).as_deref(), Some("c.bmp"));
    assert_eq!(bootlogo::selected(d.path(), &logos), Some(2));

    // 记着的那张被用户删了 ⇒ 不是"落在第一张上"，而是"没有选中项"，由调用方去回落。
    bootlogo::save(d.path(), "gone.bmp");
    assert_eq!(bootlogo::selected(d.path(), &logos), None);
}

/// 开机图必须整幅 720x480 24 位。看的是字节，不是扩展名 —— uboot 读的也是字节。
#[test]
fn a_picture_that_is_not_the_whole_screen_is_refused() {
    assert!(bootlogo::is_boot_picture(&bmp(720, 480, 24)));
    assert!(!bootlogo::is_boot_picture(&bmp(720, 481, 24)), "a row too tall");
    assert!(!bootlogo::is_boot_picture(&bmp(721, 480, 24)), "a column too wide");
    assert!(!bootlogo::is_boot_picture(&bmp(720, 480, 32)), "32 bit");
    assert!(!bootlogo::is_boot_picture(&bmp(720, 480, 8)), "palettised");
    assert!(!bootlogo::is_boot_picture(b"BM"), "a stub");
    assert!(!bootlogo::is_boot_picture(b""), "nothing at all");
}

/// **没写成的时候，卡上不能多出任何东西。** 这一屏唯一会动到 slot 以外的地方就是
/// 写分区，而"换图失败却把备份留下来了"会让人以为换成了。
#[test]
fn a_failure_leaves_the_card_alone() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let dir = boot_dir(d.path());
    std::fs::write(dir.join("a.bmp"), bmp(720, 480, 24)).unwrap();
    let logos = bootlogo::list(d.path());
    if on_a_device() {
        return;
    }

    // 宿主机上没有 boot-resource 分区：报"没有这块分区"，而且一张图都没动。
    let why = bootlogo::apply(d.path(), &logos[0]).unwrap_err();
    assert_eq!(why, Failure::NoPartition, "host has no boot resource: got {why:?}");
    assert!(
        !d.path().join(bootlogo::BACKUP).exists(),
        "a failed write left a backup behind"
    );
    assert!(bootlogo::current(d.path()).is_none(), "a failed write was recorded");
}

/// 不是开机图的 bmp 直接在门口就拦掉了 —— 连分区都不去挂。
#[test]
fn a_picture_that_is_not_a_boot_picture_is_never_written() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let dir = boot_dir(d.path());
    std::fs::write(dir.join("small.bmp"), bmp(320, 240, 24)).unwrap();
    let logos = bootlogo::list(d.path());
    // 这个不用躲：规格不对的图在挂分区之前就被拦下了，压根走不到写那一步。
    assert_eq!(
        bootlogo::apply(d.path(), &logos[0]),
        Err(Failure::NotAPicture)
    );
}

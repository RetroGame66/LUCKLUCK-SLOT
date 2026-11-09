//! 开机画面（boot logo）。
//!
//! ★ 2026-10-20 六六：「卡添加 Bootlogos 目录存放开机图，用户自行存 bmp 或用工具箱转，
//!   我们先做出来浏览及切换的功能」。
//!
//! **图有两个位置，别混**：
//! - **卡上** `Bootlogos/*.bmp` —— 用户从电脑放进去的候选图。slot 只读它们，只在这里列。
//! - **分区上** `boot-resource` 的 `BOOTLOGO.BMP` —— 机器通电那一刻真显示的那张。
//!   它在 uboot 里就被读了，**比 slot 早得多**，所以写完**必须重启**才看得到新图。
//!
//! 卡上记的是**文件名**而不是编号（`System/bootlogo.txt`）：往目录里加一张、删一张、
//! 换一台机器读同一张卡，一个数字都可能指到别的东西上去，名字不会。
//!
//! ⚠️ 写分区是**这一屏唯一会动到 slot 自己以外的地方**。护栏按这个强度配：
//! 先备份原图到卡上，只覆盖分区上那一个文件，写完 `sync` 再 `umount`，长度不对就算失败。
//! 分区不存在 / 挂不上 / 只读 → 一律**什么都不写**并回报原因，绝不半途改文件。

use std::path::{Path, PathBuf};
use std::process::Command;

/// 卡根目录下放开机图的文件夹，与 `Games` / `Labels` / `Wallpapers` 同级。
pub const DIR: &str = "Bootlogos";

/// 选中的那张记在哪。`System/` 下，和 `theme.txt`、`display.txt` 同一层。
pub const STATE: &str = "System/bootlogo.txt";

/// **第一次**动分区之前，把原图留一份在卡上：分区上那张写坏了，机器就开不了机，
/// 而卡上这份是能读回来、能用工具箱塞回去的。
pub const BACKUP: &str = "System/bootlogo-original.bmp";

/// 放开机图的分区（Allwinner 的 `boot-resource`，FAT16，卷标 `Volumn`）。
///
/// ⚠️ 是这个底座（AGS-102 / BaseOS）上的分区号，**不是**所有机型的通例。
/// 别的机器上这个块设备可能根本不存在，那时 `apply` 报 [`Failure::NoPartition`]，
/// 界面上一行字告诉用户换不了，而不是假装成功。
pub const PARTITION: &str = "/dev/mmcblk0p2";

/// 自己挂的时候挂在这。
const MOUNT: &str = "/tmp/slot-bootlogo";

/// 分区上那张图的名字。8.3 全大写是官方写法，但两种拼写真机上都见过 ⇒ 一律不分大小写找。
const NAME: &str = "bootlogo.bmp";

/// 屏是 720x480，开机图必须**整幅**：宽、高、24 位。
pub const W: u32 = 720;
pub const H: u32 = 480;

/// 54 字节头 + 720·480·3。行字节 2160 正好是 4 的倍数，24 位 bmp 没有行补齐。
pub const BYTES: usize = 54 + (W as usize) * (H as usize) * 3;

/// 卡上的一张开机图。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// 带扩展名的文件名 —— 也是 `System/bootlogo.txt` 里写的那个。
    pub name: String,
    pub path: PathBuf,
}

impl Entry {
    /// 屏上显示的名字：**去掉 `.bmp`**。一列全是 bmp，后缀只是噪音。
    ///
    /// ⚠️ 名字走的是界面字体（子集化的那张面），**生僻汉字可能画不出来** ——
    /// 建议图名用字母数字，这是给用户的一句话，不是代码能补的。
    pub fn label(&self) -> &str {
        self.name
            .rsplit_once('.')
            .map_or(self.name.as_str(), |(stem, _)| stem)
    }
}

/// 卡上 `Bootlogos/` 里的开机图，按文件名排序（目录给回来的顺序不作数）。
///
/// 只收 `.bmp`：这一版不做转换（六六：用户自己存 bmp，或用工具箱转）。
/// 一张不是 bmp 的图躺在目录里**不是错误**，只是不进这一列。
pub fn list(root: &Path) -> Vec<Entry> {
    let Ok(dir) = std::fs::read_dir(root.join(DIR)) else {
        return Vec::new();
    };
    let mut out: Vec<Entry> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| !slot_store::is_hidden(p))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("bmp")))
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_string();
            Some(Entry { name, path })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 卡上记着的那张图的名字。没有文件、空文件都读作"没选过"。
pub fn current(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(STATE)).ok()?;
    let name = text.lines().next()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// 把选中的名字写回卡上。写不动（只读卡）就随它去 —— 下次开机重新选一次就是了。
pub fn save(root: &Path, name: &str) {
    let _ = std::fs::write(root.join(STATE), format!("{name}\n"));
}

/// `current` 落在第几张上。记着的那张已经不在目录里了 ⇒ `None`（调用方回落到第一张）。
pub fn selected(root: &Path, logos: &[Entry]) -> Option<usize> {
    let name = current(root)?;
    logos.iter().position(|e| e.name == name)
}

/// 一张能当开机图的 bmp：**只看字节，不看扩展名** —— 扩展名是给人看的，uboot 读的是字节。
///
/// 720x480、24 位、不压缩、长度够放下整幅。长度用"够"而不是"正好"：
/// 有的编码工具在像素后面留了别的块，那些字节 uboot 不会读，但为此拒绝一张好图不值。
///
/// ⚠️ **查不出"图本身画错了"**：顶部留一截黑边、内容下移这类，字节上完全合法，
/// 只有真机开机才看得见 —— 2026-09-14 那次 51px 黑边就是这样。
pub fn is_boot_picture(bytes: &[u8]) -> bool {
    if bytes.len() < BYTES || &bytes[0..2] != b"BM" {
        return false;
    }
    let width = le_i32(bytes, 18);
    let height = le_i32(bytes, 22);
    let bpp = le_u16(bytes, 28);
    let compression = le_u32(bytes, 30);
    width == W as i32 && height.abs() == H as i32 && bpp == 24 && compression == 0
}

/// 为什么这一张没换成。给日志用，也给屏上那行字用。
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Failure {
    /// 卡上那张不是 720x480 24 位的 bmp（或读不出来）。
    NotAPicture,
    /// 这台机器上没有 `boot-resource` 分区 ⇒ 这个功能在这里根本不成立。
    NoPartition,
    /// 分区挂不上（不是 root、内核没 vfat、挂载点被占）。
    Mount,
    /// 分区挂上了，里面没有 bootlogo —— 那就没有"该被覆盖的那张图"。
    NoTarget,
    /// 原图没能备份到卡上 ⇒ **不往下写**（宁可不换，不留一个没有退路的状态）。
    Backup,
    /// 覆盖写失败（分区只读、满了）。
    Write,
    /// 写完了，读回来的长度和写进去的不一样。
    Verify,
}

/// 把 `entry` 写到分区上，成为机器下一次开机的画面。
///
/// 顺序是护栏的全部：**先备份 → 再写 → 再核对 → 最后 sync/umount**。
/// 任何一步不过就停在那里返回，分区保持原样（除了可能多出一次失败的无害挂载）。
pub fn apply(root: &Path, entry: &Entry) -> Result<(), Failure> {
    let bytes = std::fs::read(&entry.path).map_err(|_| Failure::NotAPicture)?;
    if !is_boot_picture(&bytes) {
        return Err(Failure::NotAPicture);
    }
    if !Path::new(PARTITION).exists() {
        return Err(Failure::NoPartition);
    }
    let (mnt, ours) = mount_rw()?;
    let result = write_into(&mnt, root, &bytes);
    if result.is_ok() {
        let _ = Command::new("sync").status();
    }
    if ours {
        let _ = Command::new("umount").arg(&mnt).status();
    }
    result
}

/// 挂上（或找到）一个**可写**的挂载点，第二个值是"是不是我们挂的"（是才要 umount）。
///
/// 已经挂着的挂载点**一律复用**：同一块 FAT 挂两次会有两份 cache，
/// 一边写、另一边 umount 时把旧 cache 刷回去，刚写的图就被盖掉了 —— 这是工具箱踩过的。
/// 挂着的那个是只读的 ⇒ 试着 `remount,rw`，仍然不行才算失败。
fn mount_rw() -> Result<(PathBuf, bool), Failure> {
    if let Some(mnt) = mounted_at() {
        return if mounted_rw(&mnt) {
            Ok((mnt, false))
        } else {
            remount_rw(&mnt).then_some((mnt, false)).ok_or(Failure::Mount)
        };
    }
    let _ = Command::new("mkdir").args(["-p", MOUNT]).status();
    let _ = Command::new("mount")
        .args(["-t", "vfat", "-o", "rw", PARTITION, MOUNT])
        .status();
    // 挂载命令的退出码在这台机器上不能信（busybox 对某些失败照样回 0），
    // 挂完直接去 /proc/mounts 里核对 —— 猜错就会"报挂上了、其实没挂"，后面一路静默失败。
    mounted_at().map(|m| (m, true)).ok_or(Failure::Mount)
}

/// 分区现在挂在哪，没挂就是 `None`。
fn mounted_at() -> Option<PathBuf> {
    let text = std::fs::read_to_string("/proc/mounts").ok()?;
    text.lines().find_map(|line| {
        let mut cols = line.split_whitespace();
        let (device, mount) = (cols.next()?, cols.next()?);
        (device == PARTITION).then(|| PathBuf::from(mount))
    })
}

/// 那个挂载点的选项里有没有 `rw`。
fn mounted_rw(mnt: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string("/proc/mounts") else {
        return false;
    };
    let want = mnt.to_string_lossy().to_string();
    text.lines().any(|line| {
        let cols: Vec<&str> = line.split_whitespace().collect();
        cols.len() >= 4
            && cols[1] == want
            && cols[3].split(',').any(|o| o == "rw")
    })
}

fn remount_rw(mnt: &Path) -> bool {
    let ok = Command::new("mount")
        .args(["-o", "remount,rw"])
        .arg(mnt)
        .status()
        .is_ok_and(|s| s.success());
    ok && mounted_rw(mnt)
}

/// 在挂载点里找到那张图（不分大小写），备份，覆盖，核对长度。
fn write_into(mnt: &Path, root: &Path, bytes: &[u8]) -> Result<(), Failure> {
    let Ok(dir) = std::fs::read_dir(mnt) else {
        return Err(Failure::NoTarget);
    };
    let mut target: Option<PathBuf> = None;
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name == NAME {
            target = Some(entry.path());
            break;
        }
    }
    let Some(target) = target else {
        return Err(Failure::NoTarget);
    };
    // 备份只做一次：卡上已经有了就说明这张卡已经动过分区，那份才是"原厂"的。
    let backup = root.join(BACKUP);
    if !backup.exists() {
        std::fs::copy(&target, &backup).map_err(|_| Failure::Backup)?;
    }
    std::fs::write(&target, bytes).map_err(|_| Failure::Write)?;
    let got = std::fs::metadata(&target).map(|m| m.len() as usize).unwrap_or(0);
    (got == bytes.len()).then_some(()).ok_or(Failure::Verify)
}

fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn le_i32(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

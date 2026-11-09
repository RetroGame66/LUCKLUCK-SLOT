/// Cut from a recording of the real thing by `slot-sfxcut`, mono signed 16 bit little endian
/// at 48 kHz. One continuous take per direction, played as recorded: no stretching, no
/// envelope, nothing joined. Everything the sound needs to do it already does, and the sync
/// is a question of when it starts rather than what is done to it.
const INSERT: &[u8] = include_bytes!("../../assets/insert.pcm");
const EJECT: &[u8] = include_bytes!("../../assets/eject.pcm");

/// ★ 2026-10-26（六六）：UI 提示音的**成品**素材，由 `_tools/make_ui_sfx.py` 合成。
///
/// 规格与来由见工作区根 `slot-音效清单-2026-10-26.md`：一条主线 —— **同一台机器的三个
/// 零件**（拨杆 / 机械计数器 / 继电器），干、短、有实体感，不要 chiptune 卡通音。都是
/// 48 kHz mono i16 LE 裸 PCM，峰值 −6 dBFS，头尾各 1.5 / 3 ms 渐变，**没有尾部静音**
/// （`tail()` 由文件长度派生，凑长度会改动画时序）。
///
/// ★ **规矩**：新增音效 = 加一个 `Sfx` 成员 + 在 `asset()` 里指一行素材 + 在调用点
/// 派发一次，**不动 `render`/`resampled`**（那两个是管线，音效多少条都一样）。
const BLIP: Option<&[u8]> = Some(include_bytes!("../../assets/blip.pcm"));
const TICK: Option<&[u8]> = Some(include_bytes!("../../assets/tick.pcm"));
const CLICK: Option<&[u8]> = Some(include_bytes!("../../assets/click.pcm"));
const REFUSE: Option<&[u8]> = Some(include_bytes!("../../assets/refuse.pcm"));
/// ★ 2026-10-26：**光盘不是卡带。** 托盘/吸入 + 伺服起停，和卡带滑轨是两条不同的机械；
/// 共用 `insert`/`eject` 那两条真实录音，会听起来像这台机器装错了零件。
const DISC_IN: Option<&[u8]> = Some(include_bytes!("../../assets/disc_in.pcm"));
const DISC_OUT: Option<&[u8]> = Some(include_bytes!("../../assets/disc_out.pcm"));
/// 开机一条。**−12 dBFS**（比其余低一半）：它落在安静房间里戴耳机的人耳朵上。
const BOOT: Option<&[u8]> = Some(include_bytes!("../../assets/boot.pcm"));
const LINK_UP: Option<&[u8]> = Some(include_bytes!("../../assets/link_up.pcm"));
const LINK_DOWN: Option<&[u8]> = Some(include_bytes!("../../assets/link_down.pcm"));
const WIFI_OK: Option<&[u8]> = Some(include_bytes!("../../assets/wifi_ok.pcm"));
const WIFI_FAIL: Option<&[u8]> = Some(include_bytes!("../../assets/wifi_fail.pcm"));
/// ★ 2026-10-27：**投币不是插卡。** 币道是金属滚落 + 计币器挡片，插卡是塑料滑轨 + 触点；
/// 之前投币落进 `Sfx::Insert`，每投一次币响的都是卡带那条真实录音（`_tools/make_ui_sfx.py`
/// 的 `coin()`）。
const COIN: Option<&[u8]> = Some(include_bytes!("../../assets/coin.pcm"));
/// ★ 2026-10-27：卷闸（`slot_ui::shutter`）落 / 卷。帘片逐格咬合，落是密度递增 + 末尾砸到底，
/// 卷起不是落下的倒放（起手是链条吃力）。
const SHUTTER_DOWN: Option<&[u8]> = Some(include_bytes!("../../assets/shutter_down.pcm"));
const SHUTTER_UP: Option<&[u8]> = Some(include_bytes!("../../assets/shutter_up.pcm"));
/// ★ 2026-10-27：磁带。`TapeIn` = 盒盖合上 + 锁扣 + 压带轮落下 + 电机起动（进仓/换碟）；
/// `TapeTransport` = 机械琴键 + 电机（播放/暂停/停止/换曲，真机上本来就是同一组琴键）。
const TAPE_IN: Option<&[u8]> = Some(include_bytes!("../../assets/tape_in.pcm"));
const TAPE_TRANSPORT: Option<&[u8]> = Some(include_bytes!("../../assets/tape_transport.pcm"));
/// ★ 2026-10-27：写进闪存（即时存档 / 读档）。短的电荷泵噪声爆 + 一记轻咔，**没有上升音阶**。
const SAVE: Option<&[u8]> = Some(include_bytes!("../../assets/save.pcm"));
const BGM_PREFIX: &str = "bgm_";

const ASSET_HZ: f32 = 48_000.0;

/// A noise the frontend makes itself, as opposed to anything coming out of a core.
///
/// ★ 2026-10-22（六六）：从两个"动作"（插/拔）扩成**三类** —— 动作（插/拔）、UI 提示音
/// （`Blip`/`Tick`/`Click`）、各机种 BGM（`Bgm`）。三类共用同一条 `render` → `Ring` 管线，
/// 只是素材来源和触发时机不同。`Blip`/`Tick`/`Click` 目前素材未到位 ⇒ 静默。
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sfx {
    /// The whole of a cart going in: the shell down the rails, then the contacts.
    Insert,
    /// The whole of one coming out: the contacts letting go, then the shell back up.
    Eject,
    /// ★ 2026-10-22：一次方向/焦点移动（←→、↑↓、切行）。短提示音，**去抖 ~50ms** ——
    /// 长按连发不该是一串机枪。
    Blip,
    /// ★ 2026-10-22：一次步进（字母轮一格、设置页一行）。比 `Blip` 更像"咔哒"。
    Tick,
    /// ★ 2026-10-22：一次确认（A 插卡、菜单选中、开关落地）。
    Click,
    /// ★ 2026-10-26：**拒绝** —— 没卡的机种、坏卡、插不进去、此刻不能倒带/快进/慢放。
    /// 低、闷、短促，是"卡带塞不进去"而不是"错误提示音"。**一处发声**：`App::refuse()`
    /// 里派发，全前端十条拒绝路径共用同一个入口。
    Refuse,
    /// ★ 2026-10-26：PS1 光盘**进**（托盘吸入 + 落位 + 伺服起转，1.15 s）。
    DiscIn,
    /// ★ 2026-10-26：光盘**出**（托盘滑出 + 止位 + 电机滑行，0.60 s）。
    DiscOut,
    /// ★ 2026-10-26：开机（继电器 + 涌入电流 + 电机起转，1.00 s，−12 dBFS）。
    Boot,
    /// ★ 2026-10-26：联机建立 / 断开（对讲机式的接入与切出，−12 dBFS）。
    LinkUp,
    LinkDown,
    /// ★ 2026-10-26：Wi-Fi 连上 / 失败。ok 是同一零件的两下（第二下更轻），
    /// fail 是 `Refuse` 的加长版（两下闷卡）。
    WifiOk,
    WifiFail,
    /// ★ 2026-10-27：ARCADE / NEOGEO 的**投币**（币撞投币口框 → 币道滚落 → 计币器挡片）。
    /// 弹出也用这一条：没有"退币"那条录音，而币在金属道上走，正反都说得通。
    Coin,
    /// ★ 2026-10-27：卷闸**落**（帘片密度递增 + 砸到底，0.62 s）。
    ShutterDown,
    /// ★ 2026-10-27：卷闸**卷起**（链条起步 + 一串密撞，越往上越稀，0.56 s）。
    ShutterUp,
    /// ★ 2026-10-27：磁带**进仓**（盒盖 + 锁扣 + 压带轮 + 电机，0.34 s）。
    TapeIn,
    /// ★ 2026-10-27：走带键（播放/暂停/停止/换曲共用的那组琴键，0.20 s）。
    TapeTransport,
    /// ★ 2026-10-27：写闪存（即时存档 / 读档，0.14 s）。
    Save,
    /// ★ 2026-10-22：各机种货架 BGM 的一句。`u8` 是机种号（`System::dir_name` 的顺序：
    /// GB/GBC/GBA/SFC/FC = 0..4）；素材按 `bgm_<机种小写>.pcm` 命名，前端/会话按这个
    /// 取。**BGM 由会话层调度**（进/出货架时起停），不是一次性的 `take`。
    Bgm(u8),
}

impl Sfx {
    /// The clip this variant plays, or `None` when the card has supplied no asset for it.
    ///
    /// The two movements are the built-in recordings and always have one. Everything else is a
    /// placeholder until 六六 drops the file in — see the constants at the top.
    fn asset(self) -> Option<&'static [u8]> {
        match self {
            Sfx::Insert => Some(INSERT),
            Sfx::Eject => Some(EJECT),
            Sfx::Blip => BLIP,
            Sfx::Tick => TICK,
            Sfx::Click => CLICK,
            Sfx::Refuse => REFUSE,
            Sfx::DiscIn => DISC_IN,
            Sfx::DiscOut => DISC_OUT,
            Sfx::Boot => BOOT,
            Sfx::LinkUp => LINK_UP,
            Sfx::LinkDown => LINK_DOWN,
            Sfx::WifiOk => WIFI_OK,
            Sfx::WifiFail => WIFI_FAIL,
            Sfx::Coin => COIN,
            Sfx::ShutterDown => SHUTTER_DOWN,
            Sfx::ShutterUp => SHUTTER_UP,
            Sfx::TapeIn => TAPE_IN,
            Sfx::TapeTransport => TAPE_TRANSPORT,
            Sfx::Save => SAVE,
            // BGM is a session-scheduled loop rather than a one-shot clip, so it does not come
            // through here — `render` answers silence for it and the session plays it itself.
            Sfx::Bgm(_) => None,
        }
    }

    /// Whether this variant is a one-shot the app hands over as `sfx`.
    pub fn is_one_shot(self) -> bool {
        !matches!(self, Sfx::Bgm(_))
    }

    /// The file a per-system BGM is named by, for whoever loads it off the card. `None` for the
    /// one-shots, which are baked in.
    ///
    /// The index is `slot_store::System`'s discriminant, so the order below is the enum's:
    /// `Gba, Gb, Gbc, Sfc, Fc, Neogeo, Arcade, Md, Ps1`. Getting that order wrong swaps the
    /// GBA and Game Boy files, and nothing catches it — the name is only ever read off the
    /// card, and a card that has no BGM simply has no BGM.
    ///
    /// ★ 2026-10-25: MD and PS1 appended, which is the only place they could go.
    /// ★ **2026-10-26: `pce` appended too — the array was still nine long while the enum had
    /// ten, so a PC Engine shelf asked for `bgm_file()` and got `None`.** Nothing audible
    /// changes yet (no player is wired up at all), but the lookup is now honest.
    pub fn bgm_file(self) -> Option<String> {
        match self {
            Sfx::Bgm(sys) => {
                const DIRS: [&str; 10] = [
                    "gba", "gb", "gbc", "sfc", "fc", "neogeo", "arcade", "md", "ps1", "pce",
                ];
                DIRS.get(sys as usize).map(|d| format!("{BGM_PREFIX}{d}.pcm"))
            }
            _ => None,
        }
    }

    /// How far into the clip the contacts are. The caller starts the clip this long before
    /// the cart reaches them, which is the whole of how the two are kept together.
    ///
    /// UI blips have no contacts and no travel: they fire on the press, so their lead is zero.
    pub fn lead(self) -> f32 {
        match self {
            Sfx::Insert => 0.097,
            Sfx::Eject => 0.021,
            _ => 0.0,
        }
    }

    /// What is left of the clip after the contacts: the shell settling on the way in, the
    /// shell still moving on the way out. Nothing should cut across it.
    pub fn tail(self) -> f32 {
        let Some(pcm) = self.asset() else {
            return 0.0;
        };
        pcm.len() as f32 / 2.0 / ASSET_HZ - self.lead()
    }

    /// Interleaved stereo, mono content, at the sink's rate. A variant with no asset renders
    /// **nothing at all** rather than a zero-length block spliced into the ring — silence is
    /// the honest answer for a sound that is not there yet.
    pub fn render(self, sample_rate: u32) -> Vec<i16> {
        let Some(pcm) = self.asset() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(pcm.len() * 2);
        for v in resampled(pcm, sample_rate) {
            let s = v.clamp(-32768.0, 32767.0) as i16;
            out.push(s);
            out.push(s);
        }
        out
    }
}

/// Linear, from the asset's 48 kHz to whatever the sink runs at. Both are the same rate on
/// every platform this has met so far, so this is a safeguard rather than a hot path.
fn resampled(pcm: &[u8], sample_rate: u32) -> Vec<f32> {
    let src: Vec<f32> = pcm
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32)
        .collect();
    if sample_rate == ASSET_HZ as u32 || src.is_empty() {
        return src;
    }
    let ratio = sample_rate as f32 / ASSET_HZ;
    let n = (src.len() as f32 * ratio) as usize;
    (0..n)
        .map(|i| {
            let x = i as f32 / ratio;
            let a = x as usize;
            let f = x - a as f32;
            let b = (a + 1).min(src.len() - 1);
            src[a] * (1.0 - f) + src[b] * f
        })
        .collect()
}

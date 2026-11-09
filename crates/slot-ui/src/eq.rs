//! The deck's graphic equaliser: **10 bands, the Winamp preset set**, a biquad per band.
//!
//! # Where the numbers come from
//!
//! ★ **照搬自 VLC 的 `modules/audio_filter/equalizer_presets.h`（LGPL-2.1）**, which is itself a
//!   port of **Winamp's** preset list — the exact set every cheap MP3 player of 2001–2006
//!   shipped. Bands are `60 / 170 / 310 / 600 / 1000 / 3000 / 6000 / 12000 / 14000 / 16000` Hz,
//!   gains in dB.
//!
//!   VLC writes "all zeros" as `-1.11022e-15` (a float epsilon, not a real gain). Those are
//!   stored here as plain `0.0`.
//!
//!   ★ **The zero row is called [`Eq::Null`] here, not `Flat`.** VLC names it `Flat` and the numbers
//!   are untouched, but the ring the user walks is this deck's, and the only honest word for "no
//!   equaliser at all" is NULL — 六六 asked for exactly that entry on 2026-10-07. It is first in
//!   [`Eq::ALL`] and it is the default, so a deck nobody has touched is honestly bypassed.
//!
//! ⚠️ **The 14 kHz and 16 kHz bands are nearly dead on this device's speaker.** They are kept
//! because they are part of the preset table, and dropping them would make the numbers lie about
//! where they came from. They cost two biquads each and change nothing you can hear — which is
//! the honest price of quoting a source rather than inventing one.
//!
//! # The filter
//!
//! One **[`Biquad`] per band per channel**, cascaded (a "graphic" EQ is literally that: N peaking
//! filters in series). Coefficients from the RBJ cookbook:
//!
//! ```text
//! A     = 10^(dB/40)          // dB → amplitude for a peaking filter
//! w0    = 2π·f0/Fs
//! alpha = sin(w0) / (2Q)
//! b0 = 1 + alpha·A      b1 = -2cos(w0)     b2 = 1 - alpha·A
//! a0 = 1 + alpha/A      a1 = -2cos(w0)     a2 = 1 - alpha/A
//! ```
//!
//! `Q = 1.0` for every band **except the top three** — a graphic EQ's sliders are ~1.4 octaves
//! wide, and that is what the original tables were written against. The top three (12/14/16 kHz)
//! are narrowed to [`TOP_Q`], because at `Q = 1.0` their skirts overlap and their gains add.

/// The number of bands. **10**, which is what the preset table is written for.
pub const BANDS: usize = 10;
/// The band centre frequencies, in Hz — **VLC's `f_vlc_frequency_table_10b`**.
pub const BAND_HZ: [f32; BANDS] = [
    60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0,
];
/// Every band's Q. One number for all of them **except the top three**; see the module note.
pub const BAND_Q: f32 = 1.0;

/// 顶部三带（12 k / 14 k / 16 k，即 `BAND_HZ[TOP_Q_FROM..]`）用的 Q。
///
/// ★ **2026-10-07 六六拍板（四方案里的 P3）**。`BAND_Q = 1.0` 时这三带的带宽与它们的
/// 间距同量级 ⇒ 增益**直接相加**：`FullTreble` 在 **13 775 Hz** 叠到 +15.96 dB（限幅后
/// 仍是这个数，见 [`HEADROOM_DB`]）。收窄到 2.0 之后三带各自独立，同一预设扫出的最坏
/// 提升掉到 **+8.5 dB** —— 于是那个自动 headroom 不用再为一个**这台机器几乎发不出来**
/// 的尖峰，把整个预设的音量压低十几 dB。
///
/// ⚠️ 这是**改 EQ 形状**、不是改渲染：这三带的音色会窄一点。四方案实测见
/// `.workbuddy/memory/MEMORY-排错与工具.md` §26，工具 `_scratch/eq_limit_study.py`。
pub const TOP_Q: f32 = 2.0;

/// 用 [`TOP_Q`] 的第一带的序号 —— 12000 Hz，即 `BAND_HZ[7]`。
pub const TOP_Q_FROM: usize = 7;

/// 第 `b` 带实际用的 Q：顶部三带用 [`TOP_Q`]，其余用 [`BAND_Q`]。
pub fn band_q(b: usize) -> f32 {
    if b >= TOP_Q_FROM {
        TOP_Q
    } else {
        BAND_Q
    }
}

/// 单个带允许的最大提升 / 削减，**dB**。
///
/// ★ 2026-10-07 六六：「这台机器的外放其实很一般，EQ 会爆，能把它的增益或削弱控制在一定量吗」
///   —— 这就是那个量。
///
/// 为什么非有不可：`BAND_HZ` 顶部三个带是 12000 / 14000 / 16000，而 `BAND_Q` 对**所有**带
/// 都是 1.0 ⇒ 那三个带的带宽与彼此的间距同量级，**增益直接相加**。`FullTreble` 的四段
/// （11.2 + 16 + 16 + 16 + 16.8）在 13.8 kHz 叠到 **+40.3 dB**，`Stage::apply` 末尾那句
/// 硬 `clamp` 就把它削平了 —— 听感就是"爆"。实测全表见 `_scratch/eq_headroom.py`。
///
/// ⚠️ **限幅收得住最坏值，收不住叠加本身**：最坏仍是 `上限 × 叠加段数`。所以还有**第二层**，
/// 同在 [`Stage`] 里：按每个预设实测的最坏提升**自动让出 headroom**（[`HEADROOM_DB`]），
/// 末尾再用**软膝**代替硬 `clamp`（[`soft_clip`]）。
///
/// ⚠️ 这个数**不改 `gains_db`**：那张表是 VLC 的 10 段表逐字，限幅只在
/// [`Stage::set`] 取系数时施加。原表与"我们实际用的表"分成两件事，改上限不动原表。
pub const BAND_LIMIT_DB: f32 = 6.0;

/// 允许预设最高到多高，**dB**（相对满量程）。超过的部分由 [`Stage`] 自动压掉。
///
/// ★ 2026-10-07 六六选「音质优先」：`Stage::set` 把级联最坏提升扫出来，`trim = -(peak - 这个数)`。
///   实测（`_scratch/eq_trim.py`，48 kHz）：`FullTreble` 限幅后最坏 **+15.96 dB @13.7 kHz**
///   ⇒ trim −12.96 dB；`Club` +10.58 ⇒ −7.58；`Null` 0 ⇒ 不动。
///
/// ⚠️ **trim 是标量，只降音量、不改音色** —— 那 13.7 kHz 的尖峰是 `BAND_HZ` 顶部三带
/// (12k/14k/16k) 在 `Q=1` 下互相重叠叠出来的，它是**原表形状的一部分**，trim 动不了形状。
///
/// ★ **2026-10-07（P3）：尖峰已经动了，方式是动它的形状与量它的范围** —— 顶部三带改
/// [`TOP_Q`]、扫描上限改 [`SCAN_HI`]。于是这里量到的 `peak` 变小、`trim` 变浅，
/// 提升类预设不再被压掉十几 dB。**`gains_db` 那张原表一个数都没动。**
///
/// 3 dB 是"还能好"和"别把音量削没了"之间的那个折中：留 3 dB 给峰值，`soft_clip` 兜底。
pub const HEADROOM_DB: f32 = 3.0;

/// 自动 headroom 的**扫描上限**，Hz。
///
/// ★ **2026-10-07 六六拍板（P3 的另一半）**，起因是他的原话：「EQ 现在其它之前很吵的音效
/// 被压到很小声同样也用不了」。原来的扫描一路走到 20 kHz，于是最坏点**永远是 13 775 Hz**
/// 那个尖峰 —— 而生成它的那两带（14 k / 16 k）**在这台机器的外放上几乎不发声**（见模块
/// 注释）。拿一个听不见的峰当基准，代价是每个提升类预设都被整体压低：`FullTreble` 的
/// "能听见的响度"实测掉 **−15.70 dB**。扫到 8 kHz 就只量**听得见**的那一段。
///
/// 实测四方案（`_scratch/eq_limit_study.py`，48 kHz，量粉噪加权总增益）的平均落差：
/// P0 现状 **15.70** / P1 只改这里 **10.21** / P2 只改 Q **12.22** / **P3 两者 8.79 dB**。
const SCAN_HI: f32 = 8_000.0;

/// 软膝的起点，**占满量程的比例**。这个数以下逐字通过。
const SOFT_KNEE: f32 = 0.75;

/// `i16` 的边界写成浮点，给 [`soft_clip`] 用。
const FULL_SCALE: f32 = 32768.0;

/// **软限幅**：`|x| ≤ 0.75 FS` 原样过去，之上沿一条渐近线弯到 ±FS，**永远不达到**。
///
/// 替掉的是 `v.clamp(-32768.0, 32767.0)` —— 那是**硬削平**，把过载的波形直接切出方波棱角，
/// 听感就是六六说的"爆"（`.workbuddy/memory/MEMORY-排错与工具.md` §22 量的）。软膝把同一个
/// 过载变成圆肩，谐波一样有，但不再是方波的刺。
///
/// 曲线在膝点处**值和斜率都连续**（两段都为 1），所以不会在膝点上出现新的折角：
/// `over = (a - 0.75) / 0.25`、`shaped = 0.75 + 0.25 · over/(1 + over)`，`over → ∞` 时趋近 1。
fn soft_clip(x: f32) -> f32 {
    let t = x / FULL_SCALE;
    let a = t.abs();
    if a <= SOFT_KNEE {
        return x;
    }
    let over = (a - SOFT_KNEE) / (1.0 - SOFT_KNEE);
    let shaped = SOFT_KNEE + (1.0 - SOFT_KNEE) * (over / (1.0 + over));
    (shaped * FULL_SCALE).copysign(t)
}

/// 级联的**最坏提升**：`(dB, 它出现的 Hz)`，20 Hz–[`SCAN_HI`] 对数扫。
///
/// 用**传递函数**而不是喂正弦（[`Biquad::run`] 那个状态机在 `set` 里刚重建，还没跑过）；
/// 每点算 10 个二阶节的 `|H|` 相乘，扫 300 点上下 —— 只在预设变化时跑一次，代价可以忽略。
///
/// ⚠️ 扫出的 `max` 是**这一段里的峰值**，不是某一点的值；上限是 [`SCAN_HI`] 而不是 20 kHz，
/// 理由写在那儿。
fn peak_boost_db(gains: &[f32; BANDS], rate: f32) -> (f32, f32) {
    if rate <= 0.0 {
        return (0.0, 0.0);
    }
    let mut coefs = [Biquad::default(); BANDS];
    for (b, c) in coefs.iter_mut().enumerate() {
        *c = Biquad::peaking(BAND_HZ[b], gains[b], rate, band_q(b));
    }
    let (mut peak, mut at) = (0.0f32, 0.0f32);
    let mut f = 20.0f32;
    while f <= SCAN_HI {
        let w = 2.0 * std::f32::consts::PI * f / rate;
        let (cw, sw) = (w.cos(), w.sin());
        let (c2, s2) = (2.0 * cw * cw - 1.0, 2.0 * sw * cw);
        let mut mag = 1.0f32;
        for c in &coefs {
            let nr = c.b0 + c.b1 * cw + c.b2 * c2;
            let ni = -(c.b1 * sw + c.b2 * s2);
            let dr = 1.0 + c.a1 * cw + c.a2 * c2;
            let di = -(c.a1 * sw + c.a2 * s2);
            mag *= (nr * nr + ni * ni).sqrt() / (dr * dr + di * di).sqrt().max(1e-12);
        }
        let db = 20.0 * mag.max(1e-12).log10();
        if db > peak {
            peak = db;
            at = f;
        }
        f *= 1.02;
    }
    (peak.max(0.0), at)
}

/// A band's peaking filter, **direct form I**, normalised by `a0` at build time so the inner loop
/// is five multiplies and no divides.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    /// A peaking filter at `hz` with `db` of gain and quality `q`, for a stream at `rate`.
    /// **A gain of 0 is a bypass**, and the caller relies on that: an all-zero preset costs
    /// nothing. `q` comes from [`band_q`] — the top three bands are narrower than the rest.
    fn peaking(hz: f32, db: f32, rate: f32, q: f32) -> Self {
        let a = 10f32.powf(db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * hz / rate.max(1.0);
        let alpha = w0.sin() / (2.0 * q);
        let cw = w0.cos();
        let (b0, b1, b2) = (1.0 + alpha * a, -2.0 * cw, 1.0 - alpha * a);
        let (a0, a1, a2) = (1.0 + alpha / a, -2.0 * cw, 1.0 - alpha / a);
        Biquad {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            ..Default::default()
        }
    }

    /// One sample through the filter.
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// The preset set — **VLC's 18, in VLC's order**. Every one carries its real dB table.
///
/// The first row is VLC's "flat"; here it is [`Eq::Null`] — see the module note. It is the
/// [`Default`], and [`Eq::ALL`] starts with it, because a deck that has never been touched
/// should honestly be bypassed rather than sitting on a preset that claims to do something.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Eq {
    #[default]
    Null,
    Classical,
    Club,
    Dance,
    FullBass,
    FullBassTreble,
    FullTreble,
    Headphones,
    LargeHall,
    Live,
    Party,
    Pop,
    Reggae,
    Rock,
    Ska,
    Soft,
    SoftRock,
    Techno,
}

impl Eq {
    pub const ALL: [Eq; 18] = [
        Eq::Null,
        Eq::Classical,
        Eq::Club,
        Eq::Dance,
        Eq::FullBass,
        Eq::FullBassTreble,
        Eq::FullTreble,
        Eq::Headphones,
        Eq::LargeHall,
        Eq::Live,
        Eq::Party,
        Eq::Pop,
        Eq::Reggae,
        Eq::Rock,
        Eq::Ska,
        Eq::Soft,
        Eq::SoftRock,
        Eq::Techno,
    ];

    pub fn next(self) -> Eq {
        let i = self as usize;
        Eq::ALL[(i + 1) % Eq::ALL.len()]
    }

    /// The preset at `i`, **wrapped** — the deck stores the preset as a `u8` and reads it back
    /// through this, so a stale or out-of-range byte still lands on a real preset rather than on
    /// undefined behaviour. `Null` is `ALL[0]`, so a freshly-zeroed deck is honestly bypassed.
    pub fn from_index(i: u8) -> Eq {
        Eq::ALL[(i as usize) % Eq::ALL.len()]
    }

    /// The name the panel prints, **short and uppercase** — the strip and the artist's row are
    /// 240 logical pixels wide and this sits next to two other things.
    pub fn name(self) -> &'static str {
        match self {
            Eq::Null => "NULL",
            Eq::Classical => "CLASSIC",
            Eq::Club => "CLUB",
            Eq::Dance => "DANCE",
            Eq::FullBass => "BASS",
            Eq::FullBassTreble => "BASS+TRBL",
            Eq::FullTreble => "TREBLE",
            Eq::Headphones => "PHONES",
            Eq::LargeHall => "HALL",
            Eq::Live => "LIVE",
            Eq::Party => "PARTY",
            Eq::Pop => "POP",
            Eq::Reggae => "REGGAE",
            Eq::Rock => "ROCK",
            Eq::Ska => "SKA",
            Eq::Soft => "SOFT",
            Eq::SoftRock => "SOFTROCK",
            Eq::Techno => "TECHNO",
        }
    }

    /// The per-band gains, in dB. **VLC's table verbatim** (its `-1.11022e-15` epsilons written
    /// here as `0.0`).
    pub fn gains_db(self) -> [f32; BANDS] {
        match self {
            Eq::Null => [0.0; BANDS],
            Eq::Classical => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -7.2, -7.2, -7.2, -9.6],
            Eq::Club => [0.0, 0.0, 8.0, 5.6, 5.6, 5.6, 3.2, 0.0, 0.0, 0.0],
            Eq::Dance => [9.6, 7.2, 2.4, 0.0, 0.0, -5.6, -7.2, -7.2, 0.0, 0.0],
            Eq::FullBass => [-8.0, 9.6, 9.6, 5.6, 1.6, -4.0, -8.0, -10.4, -11.2, -11.2],
            Eq::FullBassTreble => [7.2, 5.6, 0.0, -7.2, -4.8, 1.6, 8.0, 11.2, 12.0, 12.0],
            Eq::FullTreble => [-9.6, -9.6, -9.6, -4.0, 2.4, 11.2, 16.0, 16.0, 16.0, 16.8],
            Eq::Headphones => [4.8, 11.2, 5.6, -3.2, -2.4, 1.6, 4.8, 9.6, 12.8, 14.4],
            Eq::LargeHall => [10.4, 10.4, 5.6, 5.6, 0.0, -4.8, -4.8, -4.8, 0.0, 0.0],
            Eq::Live => [-4.8, 0.0, 4.0, 5.6, 5.6, 5.6, 4.0, 2.4, 2.4, 2.4],
            Eq::Party => [7.2, 7.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 7.2, 7.2],
            Eq::Pop => [-1.6, 4.8, 7.2, 8.0, 5.6, 0.0, -2.4, -2.4, -1.6, -1.6],
            Eq::Reggae => [0.0, 0.0, 0.0, -5.6, 0.0, 6.4, 6.4, 0.0, 0.0, 0.0],
            Eq::Rock => [8.0, 4.8, -5.6, -8.0, -3.2, 4.0, 8.8, 11.2, 11.2, 11.2],
            Eq::Ska => [-2.4, -4.8, -4.0, 0.0, 4.0, 5.6, 8.8, 9.6, 11.2, 9.6],
            Eq::Soft => [4.8, 1.6, 0.0, -2.4, 0.0, 4.0, 8.0, 9.6, 11.2, 12.0],
            Eq::SoftRock => [4.0, 4.0, 2.4, 0.0, -4.0, -5.6, -3.2, 0.0, 2.4, 8.8],
            Eq::Techno => [8.0, 5.6, 0.0, -5.6, -4.8, 0.0, 8.0, 9.6, 9.6, 8.8],
        }
    }

    /// 表里的增益，**按 [`BAND_LIMIT_DB`] 限幅之后** —— 这是 `Stage` 实际拿来建滤波器的那张表。
    ///
    /// 与 [`Self::gains_db`] 分成两个函数是有意的：那两个数（逐字原表 / 我们的上限）将来
    /// 会各自变，而"这张表是 VLC 的"这句话一天不假，就不能让人在 `gains_db` 里找不到它。
    /// **谁在用哪一张，看调用点：`Stage::set` 用的是这一张。**
    pub fn limited_gains_db(self) -> [f32; BANDS] {
        let mut g = self.gains_db();
        for v in g.iter_mut() {
            *v = v.clamp(-BAND_LIMIT_DB, BAND_LIMIT_DB);
        }
        g
    }

    /// The preset's preamp, in dB — **part of the table, not a guess.** VLC applies it so a
    /// preset that boosts does not clip; a `FullTreble` at +16 dB without its 3 dB preamp pull
    /// would be a brick wall.
    pub fn preamp_db(self) -> f32 {
        match self {
            Eq::Null => 12.0,
            Eq::Classical => 12.0,
            Eq::Club => 6.0,
            Eq::Dance => 5.0,
            Eq::FullBass => 5.0,
            Eq::FullBassTreble => 4.0,
            Eq::FullTreble => 3.0,
            Eq::Headphones => 4.0,
            Eq::LargeHall => 5.0,
            Eq::Live => 7.0,
            Eq::Party => 6.0,
            Eq::Pop => 6.0,
            Eq::Reggae => 8.0,
            Eq::Rock => 5.0,
            Eq::Ska => 6.0,
            Eq::Soft => 5.0,
            Eq::SoftRock => 7.0,
            Eq::Techno => 5.0,
        }
    }
}

/// A live equaliser: one biquad per band per channel, plus the preset it is currently set to.
///
/// **Created per stream, not per block** — the filter's state (the last two samples in and out)
/// is the filter. Rebuilding it every packet would restart the tail and click.
pub struct Stage {
    rate: f32,
    preset: Option<Eq>,
    /// `[channel][band]`.
    cells: Vec<[Biquad; BANDS]>,
    /// **The automatic headroom**, linear — what every sample is scaled by *before* the cascade.
    ///
    /// `1.0` for `Null` (all gains zero ⇒ no peak ⇒ nothing to give up). See [`HEADROOM_DB`].
    /// Scaling the input is the same as scaling the output (the cascade is linear), and it costs
    /// one multiply per sample either way.
    trim: f32,
}

impl Stage {
    /// A stage for `channels` channels. The preset is unset until [`Stage::set`] is called with a
    /// rate, which is when the coefficients are actually built.
    pub fn new(channels: usize) -> Self {
        Stage {
            rate: 0.0,
            preset: None,
            cells: vec![[Biquad::default(); BANDS]; channels.max(1)],
            trim: 1.0,
        }
    }

    /// Retune to `preset` at `rate`. **Cheap when nothing changed** — the deck calls this once
    /// per block and the usual answer is "still Rock, still 44100", which costs ten compares.
    pub fn set(&mut self, preset: Eq, rate: u32) {
        if self.preset == Some(preset) && self.rate == rate as f32 {
            return;
        }
        self.preset = Some(preset);
        self.rate = rate as f32;
        let gains = preset.limited_gains_db();
        // ⚠️ **The table's preamp is deliberately not applied.** VLC's `f_preamp` is 12.0 for
        // `flat` and 3.0 for `fulltreble` (which already pushes +16 dB), and I could not settle
        // from the source whether that is dB or a slider offset — a +12 dB flat "gain" would
        // clip the deck on every preset. The band gains are the part that makes Rock sound like
        // Rock; a hand-written global trim is a volume knob wearing a costume, and the deck
        // already has one.
        //
        // ★ **What *is* applied is a derived trim** — see `HEADROOM_DB`. That is a different animal
        //   from the table's preamp: it is not a taste, it is whatever keeps this preset's own
        //   worst-case boost under the ceiling, measured from the coefficients just built. A preset
        //   that does not boost (`Null`, `Classical`) is not touched at all.
        //
        // ★ **The band gains are limited** — see `BAND_LIMIT_DB`. The table is VLC's verbatim and
        //   stacks its four top bands to +40 dB at 13.8 kHz, which the old hard clamp at the end of
        //   `apply` then squared off; that square wave is the "爆" 六六 heard. The limit is
        //   applied *here* and not inside `gains_db`, so the table stays the table.
        let trim_db = (peak_boost_db(&gains, self.rate).0 - HEADROOM_DB).max(0.0);
        self.trim = 10f32.powf(-trim_db / 20.0);
        for ch in self.cells.iter_mut() {
            for (b, cell) in ch.iter_mut().enumerate() {
                *cell = Biquad::peaking(BAND_HZ[b], gains[b], self.rate, band_q(b));
            }
        }
    }

    /// Run one interleaved block through the cascade.
    ///
    /// **`Null` is a no-op** rather than ten filters that happen to do nothing: the deck pays for
    /// the EQ only when the user has actually turned it away from NULL.
    ///
    /// The tail of the chain is [`soft_clip`], not a `clamp` — an over-driven preset rounds off
    /// instead of being squared. It should rarely engage: `set` has already given the preset the
    /// headroom it needs, and the soft knee only catches what program material still pushes past.
    pub fn apply(&mut self, samples: &mut [i16], channels: usize) {
        let Some(preset) = self.preset else {
            return;
        };
        if channels == 0 {
            return;
        }
        if preset == Eq::Null {
            return;
        }
        for (i, s) in samples.iter_mut().enumerate() {
            let ch = i % channels;
            let mut v = *s as f32 * self.trim;
            for cell in self.cells[ch].iter_mut() {
                v = cell.run(v);
            }
            *s = soft_clip(v) as i16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sine at `hz`, one second's worth, at `rate`, with peak amplitude `amp`.
    fn tone_amp(hz: f32, rate: u32, amp: f32) -> Vec<i16> {
        (0..rate)
            .map(|i| {
                let t = i as f32 / rate as f32;
                (amp * (2.0 * std::f32::consts::PI * hz * t).sin()) as i16
            })
            .collect()
    }

    /// A sine at `hz`, one second's worth, at `rate`. **Loud** (peak 8000) — louder than the
    /// filter's own settling, so the "lifts the band" tests below are not measuring noise.
    fn tone(hz: f32, rate: u32) -> Vec<i16> {
        tone_amp(hz, rate, 8000.0)
    }

    /// The RMS of a block, in the same units as the samples.
    fn rms(v: &[i16]) -> f32 {
        if v.is_empty() {
            return 0.0;
        }
        (v.iter().map(|s| (*s as f32) * (*s as f32)).sum::<f32>() / v.len() as f32).sqrt()
    }

    /// One tone through a fresh stage set to `preset`. **A fresh stage every time** — the biquad
    /// state is per-block history, and carrying it from the previous tone would measure the tail
    /// of the wrong signal.
    ///
    /// ⚠️ **Mono, and it has to be.** `tone()` builds *one* channel's worth of samples; handing it
    /// to a stereo `apply` interleaves it, so each channel receives a **2× decimated** copy while
    /// the coefficients were still designed for `rate` — the filter then sees the tone at **double
    /// its frequency** (a 1 kHz probe measures 2 kHz, a 60 Hz probe 120 Hz). The old "lifts the
    /// band" tests were written against those doubled numbers and happened to pass. One channel,
    /// and the measurement is what the maths says it is.
    fn through(preset: Eq, hz: f32, rate: u32) -> Vec<i16> {
        let mut v = tone(hz, rate);
        let mut st = Stage::new(1);
        st.set(preset, rate);
        st.apply(&mut v, 1);
        v
    }

    /// **NULL is a genuine no-op** — not ten filters that happen to cancel. This is the line
    /// that keeps the EQ from costing anything when nobody has touched it.
    #[test]
    fn null_passes_audio_through_untouched() {
        let rate = 48_000;
        let src = tone(440.0, rate);
        let mut out = src.clone();
        let mut st = Stage::new(2);
        st.set(Eq::Null, rate);
        st.apply(&mut out, 2);
        assert_eq!(out, src, "NULL 应当整块原样过去");
    }

    /// ★ **The stage gives a boosting preset its own headroom, and never squares it off.**
    ///
    /// Two separate things are being守 here:
    /// * `trim` is *derived from this preset's own coefficients* — not a taste, not a constant
    ///   baked per preset. Measured with `_scratch/eq_limit_study.py` under the **P3** rules
    ///   ([`TOP_Q`] + [`SCAN_HI`]): `FullTreble` limited peaks at about **+8.5 dB** ⇒ `trim`
    ///   −5.5 dB. Before P3 the same scan reported **+15.96 dB @ 13 775 Hz** ⇒ `trim` −12.96 dB,
    ///   which is the 「被压到很小声」 六六 complained about.
    /// * the tail is a **soft knee**, so a 0.9 FS tone driven past full scale comes out rounded
    ///   instead of flat-topped: the old `clamp` produced samples sitting exactly on 32767, and
    ///   that square wave is the "爆" 六六 heard.
    #[test]
    fn a_boosting_preset_gives_up_headroom_instead_of_squaring_off() {
        let rate = 48_000;
        let (peak_db, at) = peak_boost_db(&Eq::FullTreble.limited_gains_db(), rate as f32);
        assert!(
            peak_db > 8.0,
            "限幅后 FullTreble 的最坏提升应 >8 dB，实测 {peak_db:.2} @ {at:.0} Hz"
        );
        let trim = trim_of(Eq::FullTreble, rate);
        // ★ **这条下界就是 P3 的守门人**：老规矩（20 kHz 扫、Q=1）下 trim 会掉到 0.22 上下
        //   —— 那正是"其它音效被压到很小声"。只扫 ≤8 kHz + 顶部收窄之后只让 5.5 dB。
        assert!(
            (0.4..0.7).contains(&trim),
            "P3 下 trim 只应让掉 5.5 dB 上下，实测 {trim}（peak {peak_db:.2} dB）"
        );

        // 0.9 满量程，喂在**峰值频率**上：软膝之下、永远差一点到满量程。（单通道，见 `through`。）
        let mut st = Stage::new(1);
        st.set(Eq::FullTreble, rate);
        let mut buf = tone_amp(at, rate, 0.9 * FULL_SCALE);
        st.apply(&mut buf, 1);
        let top = buf.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(top < 32767, "软限幅不应当贴到满量程，实测峰值 {top}");
        // 而且确实进过膝区（否则这条测试只是在量一条直线）。
        assert!(top > (SOFT_KNEE * FULL_SCALE) as u16, "应当过了膝点，实测峰值 {top}");
    }

    /// `Stage` 的 `trim` —— 那个字段是私有的，测试从一条新 stage 上取。
    fn trim_of(preset: Eq, rate: u32) -> f32 {
        let mut st = Stage::new(1);
        st.set(preset, rate);
        st.trim
    }

    /// ★ **P3 真的把最坏提升压下来了，而且没有动原表。**
    ///
    /// 三件事一起守：① 顶部三带的 Q 是 [`TOP_Q`] 而其余仍是 [`BAND_Q`]；② 扫描只到
    /// [`SCAN_HI`]；③ [`Eq::gains_db`] 那张 VLC 表**逐字没变**（`FullTreble` 顶部仍是 16.0 /
    /// 16.8，不是被限幅后的 6.0）—— 限幅与收窄都发生在建滤波器的时候，不在表里。
    #[test]
    fn the_top_bands_are_narrowed_and_the_table_is_untouched() {
        assert_eq!(band_q(0), BAND_Q);
        assert_eq!(band_q(TOP_Q_FROM - 1), BAND_Q);
        assert_eq!(band_q(TOP_Q_FROM), TOP_Q);
        assert_eq!(band_q(BANDS - 1), TOP_Q);
        // 顶部三带比常规带窄——两个常量之间的关系，编译期就断言掉。
        const _: () = assert!(TOP_Q > BAND_Q);
        assert_eq!(BAND_HZ[TOP_Q_FROM], 12_000.0, "TOP_Q_FROM 应当是 12 kHz 那一带");

        // 原表：`FullTreble` 顶部 16.0 / 16.0 / 16.8 —— 收窄和限幅都不许碰它。
        let raw = Eq::FullTreble.gains_db();
        assert_eq!(&raw[7..], &[16.0, 16.0, 16.8], "VLC 原表不许被改");
        // 实际建表的那张被限到 ±6。
        let limited = Eq::FullTreble.limited_gains_db();
        assert_eq!(limited[7], BAND_LIMIT_DB);

        let (wide, _) = peak_boost_db(&limited, 48_000.0);
        // 同一张（已限幅的）表，把顶部三带放回 Q=1 —— 老形状的最坏提升明显更高。
        let mut coefs = [Biquad::default(); BANDS];
        for (b, c) in coefs.iter_mut().enumerate() {
            *c = Biquad::peaking(BAND_HZ[b], limited[b], 48_000.0, BAND_Q);
        }
        let mut old_peak = 0.0f32;
        let mut f = 20.0f32;
        while f <= 20_000.0 {
            let w = 2.0 * std::f32::consts::PI * f / 48_000.0;
            let (cw, sw) = (w.cos(), w.sin());
            let (c2, s2) = (2.0 * cw * cw - 1.0, 2.0 * sw * cw);
            let mut mag = 1.0f32;
            for c in &coefs {
                let nr = c.b0 + c.b1 * cw + c.b2 * c2;
                let ni = -(c.b1 * sw + c.b2 * s2);
                let dr = 1.0 + c.a1 * cw + c.a2 * c2;
                let di = -(c.a1 * sw + c.a2 * s2);
                mag *= (nr * nr + ni * ni).sqrt() / (dr * dr + di * di).sqrt().max(1e-12);
            }
            old_peak = old_peak.max(20.0 * mag.max(1e-12).log10());
            f *= 1.02;
        }
        assert!(
            old_peak > wide + 4.0,
            "老形状（Q=1、扫到 20 kHz）应当明显更高：{old_peak:.2} vs {wide:.2} dB"
        );
    }

    /// **Rock puts 60 Hz above 1 kHz.** The preset says +8.0 dB on the first band; if the
    /// coefficient maths is wrong (a sign, a missing `a0` division) the ratio flips or flattens.
    ///
    /// 比的是**同一预设下两个频率**，不比"输出 vs 原始输入"：`trim` 是标量，会在比值里约掉，
    /// 所以这条判据既能守系数、又不会被自动 headroom 的改动弄红。实测比值（`eq_trim.py`）
    /// 60 Hz −6.50 dB 对 1 kHz −18.63 dB ⇒ **4.0×**。
    #[test]
    fn rock_lifts_the_bottom_band() {
        let rate = 48_000;
        let (low, mid) = (rms(&through(Eq::Rock, 60.0, rate)), rms(&through(Eq::Rock, 1000.0, rate)));
        assert!(low > mid * 2.5, "Rock 应当把 60 Hz 抬在 1 kHz 之上：{low} vs {mid}");
    }

    /// **FullTreble puts 16 kHz above 1 kHz** — the band Rock only touches at +11.2, so the two
    /// presets have to be distinguishable or the whole table is decorative. 实测 **4.41×**。
    #[test]
    fn full_treble_lifts_the_top_band() {
        let rate = 48_000;
        let (top, mid) = (
            rms(&through(Eq::FullTreble, 16_000.0, rate)),
            rms(&through(Eq::FullTreble, 1000.0, rate)),
        );
        assert!(top > mid * 2.5, "FullTreble 应当把 16 kHz 抬在 1 kHz 之上：{top} vs {mid}");
    }

    /// **The cascade must not diverge.** A biquad with a normalisation bug feeds its own output
    /// back and reaches ±∞ within a few hundred samples; one second of that is silence or a
    /// brick wall, and either is exactly what an EQ bug sounds like.
    ///
    /// We test the **raw cascade** (bypassing `trim` and `soft_clip`, either of which would hide a
    /// divergence behind a saturated rail) on a *small* tone, so a legitimate boost stays well
    /// under the rail and we are measuring the filter, not the limiter. The gains here are the
    /// **limited** ones, so the worst legitimate peak is around 6× rather than the raw table's
    /// 130×; a diverged filter would be astronomically larger than either.
    #[test]
    fn every_preset_stays_in_range() {
        let rate = 48_000;
        let src = tone_amp(1000.0, rate, 1000.0);
        for preset in Eq::ALL {
            let mut st = Stage::new(2);
            st.set(preset, rate);
            let mut peak = 0.0f32;
            let mut diverged = false;
            for (i, s) in src.iter().enumerate() {
                let ch = i % 2;
                let mut v = *s as f32;
                for cell in st.cells[ch].iter_mut() {
                    v = cell.run(v);
                }
                if !v.is_finite() {
                    diverged = true;
                    break;
                }
                peak = peak.max(v.abs());
            }
            assert!(!diverged, "{preset:?} 级联发散（NaN/∞）");
            assert!(peak > 0.0, "{preset:?} 把信号消掉了");
            // 限幅后稳定级联的最大增益约 6×；发散会是天文数字。
            assert!(peak < 1.0e6, "{preset:?} 增益异常（{:.0}×）", peak / 1000.0);
        }
    }

    /// `next()` walks all 18 and comes back, and the ring starts on NULL.
    #[test]
    fn the_presets_cycle() {
        let mut e = Eq::default();
        assert_eq!(e, Eq::Null, "环首应当是 NULL");
        let mut seen = Vec::new();
        for _ in 0..Eq::ALL.len() {
            seen.push(e);
            e = e.next();
        }
        assert_eq!(e, Eq::default(), "转一圈应回到起点");
        assert_eq!(seen.len(), 18);
    }
}

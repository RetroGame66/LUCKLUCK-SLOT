use crate::{Btn, Millis, RawEvent};

/// How long a *held* SELECT waits for a second key before conceding it was a plain press.
/// Generous on purpose: the only reason to ever give up is a game that wants SELECT held,
/// and 120 ms was not enough to land the second key of a chord.
pub const SELECT_CHORD_MS: Millis = 600;

/// How long a delivered tap stays down. Press and release in one batch net out to nothing,
/// because the mask is set and cleared before the core reads it.
pub const SELECT_TAP_MS: Millis = 50;
pub const MENU_TAP_MS: Millis = 250;
pub const MENU_DOUBLE_TAP_MS: Millis = 350;
pub const MENU_HOLD_MS: Millis = 1000;
/// ★ 2026-11-02（六六）：**随身听相位上 `A` 分短按/长按** —— 短按播放/暂停，长按停止。
///
/// 阈值**就是 `MENU_HOLD_MS` 这一个数**，不另立常量：卡座上两颗键的长按必须是同一段手指时间，
/// 否则"按住一秒算长按"在这颗键上成立、在那颗键上不成立，而这是没人会去记的差别。
/// 与 `SLOW_TAP_MS` 抄 `FF_DOUBLE_TAP_MS` 是同一条理由。
pub const A_HOLD_MS: Millis = MENU_HOLD_MS;
pub const FF_DOUBLE_TAP_MS: Millis = 250;
/// ★ 2026-10-20：L2 双击开慢放的窗口。**就是 `FF_DOUBLE_TAP_MS` 这一个数**，不另立常量：
///   两根肩键是同一只手的两根手指，双击锁存的手感必须是同一个，而"两个窗口后来只改了一个"
///   正是那种没人会发现的走样。
pub const SLOW_TAP_MS: Millis = FF_DOUBLE_TAP_MS;
/// How far apart the two volume keys may go down and still read as one gesture. Short,
/// because neither key is deferred waiting for it: the pair is recognised behind the presses
/// it is made of, not in front of them.
pub const MUTE_CHORD_MS: Millis = 200;
/// Well short of the PMIC's own six second cutoff (`pmu_powkey_off_time` in the device
/// tree), so slot always gets to power off gracefully before the hardware cuts the rails.
pub const POWER_HOLD_MS: Millis = 1000;

/// How long a volume key is held before the level starts running, and how fast it runs after
/// that. The press itself is the first step; this is the wait before the second, long enough
/// that a tap is never two steps and short enough that a hold does not feel stuck.
pub const VOLUME_REPEAT_DELAY_MS: Millis = 400;
pub const VOLUME_REPEAT_MS: Millis = 120;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Action {
    GbaDown(Btn),
    GbaUp(Btn),
    ShelfLeft,
    ShelfRight,
    Insert,
    Eject,
    Polaroids,
    /// **MENU 长按**。★ 2026-10-06：长按原本只发 `Eject`（弹仓），而"弹仓"只在货架/插卡
    ///   那一臂里有意义 —— 卡座屏上按长 MENU 什么都不发生。现在**同一个手势发两个动作**
    ///   （`Eject` + `MenuHold`），**由屏决定它是什么意思**：货架照旧弹仓，卡座拿它退出播放器。
    ///   手势层看不见屏，所以两个都发；这比在动作层猜"现在在哪块屏"更老实。
    MenuHold,
    SaveState,
    LoadState,
    RewindStart,
    RewindStop,
    FfStart,
    FfStop,
    /// ★ 2026-10-20：**慢放**，L2 双击，R2 双击锁快进的镜像 —— 也是锁存式：双击开，松开不关，
    ///   之后再按一次关。慢放同样是"操纵时间"，跟倒带、快进一样是联机会话里 libretro 点名
    ///   禁止的动作，所以 `App::may_slow` 那一臂在联机中把它拒掉（屏抖），`Session` 则负责
    ///   真正不把速度给下去。
    SlowStart,
    SlowStop,
    BrightnessUp,
    BrightnessDown,
    BlueLightUp,
    BlueLightDown,
    VolumeUp,
    VolumeDown,
    /// The menu, off a tap of MENU. The button's other two gestures both need a game under
    /// them, so on the shelf a tap of it meant nothing at all.
    OpenMenu,
    /// The in-game menu, off SELECT+MENU. Emitted on every screen, exactly as `OpenMenu`
    /// and `Polaroids` are: this file is blind to which screen is up, and the app is what
    /// decides where a gesture lands.
    ///
    /// Not START, which in a game is the GBA's own and the game needs it; not a bare MENU
    /// tap, which the about screen already has.
    GameMenu,
    MuteToggle,
    /// The press itself. Nothing visible hangs off it — it exists so the save state is
    /// flushed before a hold can reach the PMIC's own cutoff, which takes the rails away
    /// whatever the software wanted.
    PowerPress,
    /// A short press, delivered on release. Locking on the release rather than the press is
    /// what lets a press become a hold without dozing on the way through.
    PowerTap,
    /// The hold threshold, while the button is still down. Arms the shutdown and puts it on
    /// screen; it is `PowerOff` that commits.
    PowerHold,
    /// Released after a hold. The graceful shutdown starts here, so the screen `PowerHold`
    /// raised is on the panel for as long as the button is held.
    PowerOff,
    LidClose,
    LidOpen,
    /// SELECT+X, and what it steps depends on the machine: the **Overlay** art on a Game Boy and
    /// a Game Boy Color, and the aperture table on an Advance, the one machine that ships no art.
    /// `App`'s Phase::Playing is the fork — this only names the chord that reaches it. It is the
    /// **only** key that does it: `SELECT+R2` used to be a second one and is gone.
    MaskCycle,
    /// Cycle the in-game colour correction (GRAYSCALE / AGB-001 / NDS) on SELECT+Y,
    /// and the Game Boy's palette from the same key — a tap walks the page of palettes in hand.
    /// **长按不再开色板浏览器**（2026-10-26 六六：色板浏览器从游戏菜单进，见 `GameRow::Palette`）：
    /// 按住 SELECT+Y 只算长按，松手照样只走一次 `ColorCycle`，不会中途再开一次面板。
    ColorCycle,
    /// Toggle the cart's cheat list on SELECT+A. The list lives on the card, so this only flips
    /// it on or off; what is in it is edited on a PC.
    CheatToggle,
    /// Step the audio profile (stable / balanced / strict) on SELECT+VOL+. Emitted on every
    /// screen, like the other chords: this file is blind to which one is up, and it is the app
    /// that decides where it lands. It lands on the shelf, because applying a profile reopens
    /// the PCM and only the shelf has nothing playing to interrupt.
    AudioProfileNext,
    /// The same the other way, on SELECT+VOL-.
    AudioProfilePrev,
    /// Which way round the frontend is printed, on SELECT+START.
    ///
    /// START rather than B, L2 or R2, the other three the chord table leaves free. START is
    /// what the shelf used to open the letter-view toggle with, before that toggle was removed,
    /// so it is the one free slot already carrying the meaning "how the shelf is shown" rather
    /// than being a key that has to be taught. And it costs nothing to reach for: a bare START
    /// on the shelf opens the core picker and a bare B does nothing, so neither half of this
    /// press is a gesture the user loses by making it a chord.
    ///
    /// Emitted on every screen, like the other chords — this file is blind to which one is up.
    /// It is the app that lands it on the shelf, where the index and the case are the things
    /// being printed.
    ModeToggle,
    /// ★ 2026-11-02（六六）：**卡座上的长按 `A` = 停止**（回曲首并停下，留在这一屏）。
    ///
    /// 为什么是一个动作而不是"让 app 自己数时钟"：短按/长按必须**在按下时就分好**
    /// （短按要立刻出声，不能等一秒），而这个"等一秒看看会不会长成按住"的状态机只能长在
    /// 手势层 —— 和 `PowerTap`/`PowerHold`、`MenuHold` 同一个形状。屏能看见的是"按下去有没有
    /// 反应"，所以只能由这里先判。
    ///
    /// 只在 `a_to_deck()` 为真（卡座相位）时才可能发出；别处 `A` 一直是游戏的。
    DeckStop,
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
enum Select {
    #[default]
    Idle,
    /// Held, chord still undecided.
    Pending(Millis),
    /// A chord fired, so this press never reaches the core.
    Consumed,
    /// Passed to the core and still physically held.
    Delivered,
    /// Passed to the core after a release; the core is owed the up edge, but not in the
    /// same batch as the down.
    ReleaseDue(Millis),
}

#[derive(Default)]
pub struct Gestures {
    select: Select,
    /// Buttons swallowed by a chord, so their release is swallowed too. `u16` rather than `u8`
    /// so the chord set can grow past eight buttons (each chord holds one bit).
    chord_held: u16,
    menu_down_at: Option<Millis>,
    menu_last_tap: Option<Millis>,
    menu_eject_fired: bool,
    power_down_at: Option<Millis>,
    power_hold_fired: bool,
    vol_up_at: Option<Millis>,
    vol_down_at: Option<Millis>,
    /// When the ramp last emitted, which is `None` until it starts. Cleared by both edges, so
    /// every press begins its own ramp rather than inheriting the pace of the last one.
    vol_up_ramp: Option<Millis>,
    vol_down_ramp: Option<Millis>,
    /// The pair has already fired. Cleared only once both keys are up, so a key tapped again
    /// under a held one is not a second chord.
    mute_fired: bool,
    ff_on: bool,
    ff_latched: bool,
    /// The press that established the latch, whose release must not clear it.
    ff_latching_press: bool,
    r2_last_release: Option<Millis>,
    /// ★ 2026-10-20：L2 的锁存慢放，`ff_latched` / `ff_latching_press` 的同名镜像 —— 三个字段
    ///   凑成 R2 那一套的同一套状态机，只是另一根肩键的另一半（双击开、再按一次关）。
    slow_latched: bool,
    slow_latching_press: bool,
    l2_last_release: Option<Millis>,
    rewinding: bool,
    /// SELECT+Y 是唯一的"两次手势在一按里"的和弦：按下时不消费、松手才决定是不是走一次
    /// `ColorCycle`。**长按不再开色板浏览器**（2026-10-26 六六：浏览器从游戏菜单进，
    /// 见 `GameRow::Palette`），所以这里不再需要 `y_hold_fired` 之类的状态 ——
    /// 按住多久都只算一次 `ColorCycle`。
    y_at: Option<Millis>,
    /// Whether a cart is seated, in which case a START pressed under a held SELECT is the
    /// **game's** and must not be spent on `ModeToggle`. This crate is blind to which screen is
    /// up — that is the whole reason every chord is emitted unconditionally and the app decides
    /// where it lands — but this one cannot be fixed in the app: a chord is consumed *here*, so
    /// by the time the app saw anything the START would already have been swallowed and the game
    /// would never learn it was pressed. Hence the one piece of screen state this file is told:
    /// `Session` sets it every frame from whether a cart is in the slot.
    ///
    /// Defaults to `false`, which is the shelf's reading — a machine with nothing in it is the
    /// one the chord table was written for, and it keeps `SELECT+START` a mode toggle there.
    start_to_game: bool,
    /// ★★ 2026-10-25：**PS1 上裸 `L2`/`R2` 是游戏的。** 整个前端的传输一直是「按住 L2 倒带、
    /// 按住 R2 快进、双击锁存」，而 PlayStation 的 `L2`/`R2` 是**真游戏按键** —— 挡住它们等于
    /// 让 PS1 玩家少两个键。折中：这一位为真时，**裸按**的 `L2`/`R2` 直接当 `GbaDown` 递给核心，
    /// 传输退到 `SELECT+L2` / `SELECT+R2`（按住 SELECT 时走 `chord` 那一条老路，落到
    /// `l2_down`/`ff_down`，和以前一模一样）；为假时一切照旧，所以 GBA/GB/GBC/SFC/FC/MD 一行
    /// 代码都没变。
    ///
    /// **和 `start_to_game` 同一个理由、同一种形状**：这个 crate 不看见屏、也不看见机种，
    /// 所以只有 `Session` 能说"现在是不是 PS1 的一局"，它每帧置位一次。
    l2r2_to_game: bool,
    /// 哪几根肩键**这一按**给了游戏（`1` = `L2`，`2` = `R2`）。松开时按它决定该发 `GbaUp`
    /// 还是走传输的 `l2_up`/`ff_up` —— 不能拿 `l2r2_to_game` 现读，因为按下与松开之间
    /// 拔卡/换机种都能把它翻过去，那样核心会收到一个它从没见过的 up 边沿。
    game_shoulder: u16,
    /// ★ 2026-11-02：**卡座上 `A` 的短按/长按**，`a_to_deck` 为真时才长起来。
    ///
    /// `a_down_at` 非空 = 这一按还压着；`a_hold_fired` = 已经过了 `A_HOLD_MS` 并发过 `DeckStop`；
    /// `a_deck_press` = **这一按被延迟了**（按下没发 `GbaDown`），松手欠不欠一个 `GbaDown(A)` 由它说。
    ///
    /// ⚠️ 为什么按下要**延迟**而不是"先发 `GbaDown`、长按再补一个 `DeckStop`"：卡座上的 `A`
    ///   同时是 `A` 的动作键，先发下去会先播/暂停一次，长按就会变成"暂停→停止"两下。而卡座上
    ///   本来就没有游戏在收 `A`（`Session::act` 里没有核心就 `pad.clear()`），所以延迟**零代价**。
    a_down_at: Option<Millis>,
    a_hold_fired: bool,
    a_deck_press: bool,
    /// 卡座是不是屏上那一块，见 `set_a_to_deck`。与 `start_to_game` / `l2r2_to_game` 同一形状：
    /// 这个 crate 看不到屏，只有 `Session` 每帧告诉它一次。
    a_to_deck: bool,
}

/// Whether a held key owes a step at `now`. The wait before the first is longer than the gap
/// between the rest, so a slow tap never lands as two.
fn ramp_due(down: Option<Millis>, last: Option<Millis>, now: Millis) -> bool {
    let Some(down) = down else {
        return false;
    };
    if now.saturating_sub(down) < VOLUME_REPEAT_DELAY_MS {
        return false;
    }
    last.is_none_or(|l| now.saturating_sub(l) >= VOLUME_REPEAT_MS)
}

impl Gestures {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether fast forward is latched rather than held. Not an action: the latch is
    /// established by a release that emits nothing, and only this machine knows it happened.
    pub fn ff_latched(&self) -> bool {
        self.ff_latched
    }

    /// Tell the chord table whether `SELECT+START` belongs to a game right now. See the field:
    /// with a cart seated the press goes to the core as an ordinary START and never becomes
    /// `ModeToggle`; with none it is the shelf's theme toggle, exactly as before.
    pub fn set_start_to_game(&mut self, on: bool) {
        self.start_to_game = on;
    }

    /// Tell the shoulder arm whether a bare `L2`/`R2` belongs to a PS1 game right now. See the
    /// field: with it set the two keys are ordinary pad buttons and the transport moves to
    /// `SELECT+L2` / `SELECT+R2`; without it they are rewind and fast forward, exactly as they
    /// have always been. `Session` sets it every frame from the machine in the slot.
    pub fn set_l2r2_to_game(&mut self, on: bool) {
        self.l2r2_to_game = on;
    }

    /// ★ 2026-11-02：告诉手势层**卡座是不是现在这块屏**。为真时裸 `A` 按下先压着不发，
    /// 松手判短按（发 `GbaDown(A)`）、按满 `A_HOLD_MS` 判长按（发 `DeckStop`）；为假时一切照旧，
    /// 裸 `A` 立刻当游戏按键递下去。
    ///
    /// `Session` 每帧从 `App::is_listening()` 置位 —— 和 `set_start_to_game` 一样，因为
    /// "把按下延迟到松手"这件事一旦做错，丢的是**核心没看见的按下**，而那只能在手势层避免。
    pub fn set_a_to_deck(&mut self, on: bool) {
        self.a_to_deck = on;
    }

    /// Whether `SELECT` is **physically down**, which is not the same question as "a chord may
    /// still be forming". `Select::Pending` and `Select::Consumed` are the chord window and the
    /// keys a chord spent; `Delivered` is a SELECT the core already has, and the finger can stay
    /// on it for as long as it likes — a long-held SELECT with a shoulder pressed over it is
    /// still a two-key gesture, and the transport still owns that shoulder.
    fn select_held(&self) -> bool {
        !matches!(self.select, Select::Idle | Select::ReleaseDue(_))
    }

    pub fn feed(&mut self, ev: RawEvent, now: Millis) -> Vec<Action> {
        match ev {
            RawEvent::Down(b) => self.down(b, now),
            RawEvent::Up(b) => self.up(b, now),
        }
    }

    pub fn tick(&mut self, now: Millis) -> Vec<Action> {
        let mut out = Vec::new();
        match self.select {
            Select::Pending(d) if now.saturating_sub(d) >= SELECT_CHORD_MS => {
                self.select = Select::Delivered;
                out.push(Action::GbaDown(Btn::Select));
            }
            Select::ReleaseDue(d) if now.saturating_sub(d) >= SELECT_TAP_MS => {
                self.select = Select::Idle;
                out.push(Action::GbaUp(Btn::Select));
            }
            _ => {}
        }
        if let Some(d) = self.menu_down_at {
            if !self.menu_eject_fired && now.saturating_sub(d) >= MENU_HOLD_MS {
                self.menu_eject_fired = true;
                out.push(Action::Eject);
                out.push(Action::MenuHold);
            }
        }
        if let Some(d) = self.power_down_at {
            if !self.power_hold_fired && now.saturating_sub(d) >= POWER_HOLD_MS {
                self.power_hold_fired = true;
                out.push(Action::PowerHold);
            }
        }
        // ★ 2026-11-02：卡座上按住 `A` 满 `A_HOLD_MS` 就是"停止"。**在这里判而不是在 `down`**
        //   是因为按下那一刻还看不出这一按会长成什么；等到阈值到了、键还压着，就不用再等松手。
        if let Some(d) = self.a_down_at {
            if !self.a_hold_fired && now.saturating_sub(d) >= A_HOLD_MS {
                self.a_hold_fired = true;
                out.push(Action::DeckStop);
            }
        }
        // SELECT+Y 长按不再开浏览器（2026-10-26 六六：改从游戏菜单进）：这一整段
        // 原来在这里，现在拿掉，`y_at` 只用来标记"这一按是 SELECT+Y 的和弦"，
        // 松手时才决定是不是发一次 `ColorCycle`。
        // Held volume runs the level. Not while the pair has fired: that press was a mute,
        // and ramping under it would move the level the mute just remembered.
        if !self.mute_fired {
            if ramp_due(self.vol_up_at, self.vol_up_ramp, now) {
                self.vol_up_ramp = Some(now);
                out.push(Action::VolumeUp);
            }
            if ramp_due(self.vol_down_at, self.vol_down_ramp, now) {
                self.vol_down_ramp = Some(now);
                out.push(Action::VolumeDown);
            }
        }
        out
    }

    fn down(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        match b {
            Btn::Select => {
                self.select = Select::Pending(now);
                Vec::new()
            }
            Btn::Menu => self.menu_down(now),
            // The flush hangs off the press, because a POWER that is being held may be cut
            // by the PMIC before there is any release to see. Everything the user can
            // observe waits for the release.
            Btn::Power => {
                self.power_down_at = Some(now);
                self.power_hold_fired = false;
                vec![Action::PowerPress]
            }
            Btn::Lid => vec![Action::LidClose],
            Btn::VolUp | Btn::VolDown => {
                // Let a held SELECT turn the volume keys into the audio-profile chord, the
                // same way the other chord buttons do. Without this the volume arm swallows
                // the press before it can reach the `chord` path below, so SELECT+VOL never
                // switched profiles.
                let chording = matches!(self.select, Select::Pending(_) | Select::Consumed);
                if let (true, Some((bit, action))) = (chording, chord(b)) {
                    self.select = Select::Consumed;
                    self.chord_held |= bit;
                    return vec![action];
                }
                self.volume_press(b, now)
            }
            // L2/R2 are rewind and fast forward on their own, but a held SELECT has to be able
            // to turn them into a chord — the same check the volume keys above make, and for
            // the same reason. Without it this arm swallowed the press before it could reach
            // the `chord` path below, so SELECT+L2/R2 fired nothing at all.
            Btn::L2 | Btn::R2 => {
                let chording = matches!(self.select, Select::Pending(_) | Select::Consumed);
                if let (true, Some((bit, action))) = (chording, chord(b)) {
                    self.select = Select::Consumed;
                    self.chord_held |= bit;
                    return vec![action];
                }
                // ★★ 2026-10-25：**裸按交给 PS1 的游戏，按住 SELECT 时仍归传输。**
                //
                //   两件事让这个判断不能省、也不能写浅：
                //
                //   ① **这里不能只写 `if self.l2r2_to_game`。** `chord(L2)`/`chord(R2)` 恒为
                //      `None`（那两根键刻意不进和弦表，理由见 `chord` 末尾），上面那个
                //      `if let` 永远不成立 —— 少一个判断，PS1 上 `SELECT+L2/R2` 就再也没有
                //      倒带和快进，因为那一按会被当成裸按送给游戏。
                //   ② **判据不能只读 `Pending`/`Consumed`。** 那只是"和弦还可能成形"的窗口；
                //      长按 SELECT 过了 `SELECT_CHORD_MS` 之后是 `Delivered`（SELECT 已经给
                //      了核心），但**手指还按着**，`SELECT+L2` 必须仍然是倒带。所以"按住"读
                //      `select_held()`，那才是 SELECT 的物理状态。
                if self.l2r2_to_game && !self.select_held() {
                    self.game_shoulder |= shoulder_bit(b);
                    return vec![Action::GbaDown(b)];
                }
                // 按住 SELECT 的这一按是和弦的一半，即使 `chord` 里没有它：它给传输，而
                // SELECT 自己要像别的和弦那样被吞掉（`Consumed`），否则松开 SELECT 时会掉进
                // `select_up` 给核心一个它不该收到的 SELECT 按下。
                if self.select_held() {
                    self.select = Select::Consumed;
                }
                match b {
                    Btn::L2 => self.l2_down(now),
                    _ => self.ff_down(now),
                }
            }
            // ★ 2026-11-03（六六）：L3 按下 = 呼出亮度调节、R3 按下 = 呼出蓝光调节。先作识别验证
            //   用，有摇杆的机型后续再决定这两个键怎么分配。复用现有 HUD 路径：按一下既打开也
            //   步进一档（与 SELECT+方向 完全一致），一眼就能看出按键已被识别。
            Btn::L3 | Btn::R3 => vec![if b == Btn::L3 {
                Action::BrightnessUp
            } else {
                Action::BlueLightUp
            }],
            _ => {
                // ★ **START is the one key with a screen-dependent chord.** With a cart seated
                // its press is the game's, so it is handed over before the table is even
                // consulted — no `Select::Consumed`, no `chord_held` bit, nothing for `up` to
                // swallow: the core sees a plain down and, on release, a plain up. With none it
                // falls through and `SELECT+START` is the shelf's mode toggle. `up` needs no
                // matching arm: a chord bit that was never set is simply not found there.
                if b == Btn::Start && self.start_to_game {
                    return vec![Action::GbaDown(b)];
                }
                let chording = matches!(self.select, Select::Pending(_) | Select::Consumed);
                if let (true, Some((bit, action))) = (chording, chord(b)) {
                    self.select = Select::Consumed;
                    self.chord_held |= bit;
                    // SELECT+Y alone is not spent here. It is two gestures on one press — tapped
                    // it walks the palette page, held it opens the palette browser — and which of
                    // SELECT+Y 是"两按在同一 press 里"的和弦：按下不消费，松手才决定是不是
                    // 走一次 `ColorCycle`。长按不再开浏览器（2026-10-26 六六：从游戏菜单进），
                    // 所以这里不再需要 `y_hold_fired` 之类的状态。
                    if b == Btn::Y {
                        self.y_at = Some(now);
                        return Vec::new();
                    }
                    return vec![action];
                }
                // ★ 2026-11-02：**卡座上的裸 `A` 压着不发。** 短按/长按只能等松手或阈值才知道
                //   是哪一半，而卡座上没有核心在收 `A`（`Session::act` 没有核心就 `pad.clear()`），
                //   所以压住它不欠任何人一个按下 —— 这也是这个延迟在别处**不能**做的原因：
                //   游戏里压住 `A` 就是丢掉一次攻击。
                //
                //   位置在 `chord` 之后：按住 SELECT 再按 A 是 `CheatToggle`，那一臂在上面。
                if b == Btn::A && self.a_to_deck {
                    self.a_down_at = Some(now);
                    self.a_hold_fired = false;
                    self.a_deck_press = true;
                    return Vec::new();
                }
                vec![Action::GbaDown(b)]
            }
        }
    }

    fn up(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        match b {
            Btn::Select => self.select_up(now),
            Btn::Menu => self.menu_up(now),
            Btn::Power => self.power_up(),
            Btn::Lid => vec![Action::LidOpen],
            Btn::VolUp | Btn::VolDown => {
                // Swallow the release of a chorded volume key so it does not reach the volume
                // control or the core; mirrors the catch-all arm above.
                if let Some((bit, _)) = chord(b) {
                    if self.chord_held & bit != 0 {
                        self.chord_held &= !bit;
                        return Vec::new();
                    }
                }
                self.volume_release(b)
            }
            Btn::L2 | Btn::R2 => {
                // Swallow the release of a chorded shoulder, so it neither stops a rewind that
                // never started nor hands the core an up edge it never saw go down.
                if let Some((bit, _)) = chord(b) {
                    if self.chord_held & bit != 0 {
                        self.chord_held &= !bit;
                        return Vec::new();
                    }
                }
                // The release of a shoulder that was handed to the game. Read off
                // `game_shoulder` rather than off `l2r2_to_game`: between the press and the
                // release the cart can come out, and an `up` the core never saw go down is
                // worse than one it keeps waiting for.
                let bit = shoulder_bit(b);
                if self.game_shoulder & bit != 0 {
                    self.game_shoulder &= !bit;
                    return vec![Action::GbaUp(b)];
                }
                match b {
                    Btn::L2 => self.l2_up(now),
                    _ => self.ff_up(now),
                }
            }
            // L3/R3 是前端动作键：按下已发 HUD 动作，松开只消费掉，不再发 GbaUp 给核心。
            Btn::L3 | Btn::R3 => Vec::new(),
            _ => {
                // ★ 2026-11-02：卡座上延迟的 `A`，它的**松手**。
                //
                //   按下的那一刻没发 `GbaDown`（要等短按/长按落定）。这里补上欠的那一下：
                //   短按 ⇒ 补发 `GbaDown(A)`，app 层的 `Phase::Listening` 把它读成播放/暂停；
                //   长按 ⇒ `DeckStop` 已经在阈值那一刻发过了，松手就不再欠短按，返回空。
                //
                //   只清 A 自己的三个字段，chord 的 A（`SELECT+A`=金手指）不走这条：那一下在
                //   `down` 的 chord 路径里就被 `chord_held` 记了，`a_deck_press` 不会被设上。
                if b == Btn::A && self.a_deck_press {
                    let was_hold = self.a_hold_fired;
                    self.a_down_at = None;
                    self.a_hold_fired = false;
                    self.a_deck_press = false;
                    if was_hold {
                        return Vec::new();
                    }
                    return vec![Action::GbaDown(b)];
                }
                // SELECT+Y 的松手：按住多久都只算一次 `ColorCycle`（长按不再开浏览器，
                // 2026-10-26 六六：色板浏览器从游戏菜单进）。
                // 判据是"这一按本来就是 SELECT+Y 的和弦"：裸 Y 归游戏、货架上的裸 Y 用来
                // 清机种 hold，都不能被这一支吃。
                if b == Btn::Y && self.y_at.is_some() {
                    if let Some((bit, _)) = chord(b) {
                        self.chord_held &= !bit;
                    }
                    self.y_at = None;
                    return vec![Action::ColorCycle];
                }
                if let Some((bit, _)) = chord(b) {
                    if self.chord_held & bit != 0 {
                        self.chord_held &= !bit;
                        return Vec::new();
                    }
                }
                vec![Action::GbaUp(b)]
            }
        }
    }

    fn select_up(&mut self, now: Millis) -> Vec<Action> {
        match std::mem::take(&mut self.select) {
            // The release settles it: no chord can follow, so the game gets the press now
            // rather than waiting out a window that can no longer produce one.
            Select::Pending(_) => {
                self.select = Select::ReleaseDue(now);
                vec![Action::GbaDown(Btn::Select)]
            }
            Select::Delivered => vec![Action::GbaUp(Btn::Select)],
            _ => Vec::new(),
        }
    }

    /// MENU is dispatched by name from `down`, ahead of the `_` arm that reads the `chord`
    /// table, so a row there would never be looked at. Its one chord lives here instead.
    ///
    /// Ahead of the double tap check, and the order is load-bearing: behind it, a SELECT+MENU
    /// that follows a recent tap opens the switcher rather than the menu.
    fn menu_down(&mut self, now: Millis) -> Vec<Action> {
        if matches!(self.select, Select::Pending(_) | Select::Consumed) {
            // So `select_up` emits no stray press for a SELECT the game never gets.
            self.select = Select::Consumed;
            // The chord is the whole gesture, and these two are what make it one. Clearing
            // the hold stops this press also arming an eject — and, because `menu_up` reads
            // that same field to decide the press ever happened, it is what makes the release
            // silent rather than a tap landing on top of the menu it just opened. Clearing
            // the tap stops the press standing as half of a double tap in either direction:
            // this one is not the first half of one, and it must not complete one either.
            self.menu_down_at = None;
            self.menu_last_tap = None;
            return vec![Action::GameMenu];
        }
        if let Some(tap) = self.menu_last_tap {
            if now.saturating_sub(tap) <= MENU_DOUBLE_TAP_MS {
                self.menu_last_tap = None;
                // A double tap acts on the second press, so that press cannot also arm an eject.
                self.menu_down_at = None;
                return vec![Action::Polaroids];
            }
        }
        self.menu_down_at = Some(now);
        self.menu_eject_fired = false;
        Vec::new()
    }

    fn menu_up(&mut self, now: Millis) -> Vec<Action> {
        // No armed press to release, because `menu_down` already spent this one: on a
        // double tap, or on the SELECT+MENU chord. Either way, letting go of it is not a
        // second gesture on top of what it already did.
        let Some(d) = self.menu_down_at.take() else {
            return Vec::new();
        };
        let ejected = self.menu_eject_fired;
        self.menu_eject_fired = false;
        let tapped = !ejected && now.saturating_sub(d) < MENU_TAP_MS;
        self.menu_last_tap = tapped.then_some(now);
        // On the release rather than after the double tap window closes: waiting would put a
        // third of a second between the press and the screen, to serve a gesture that means
        // nothing on the shelf anyway. A second tap inside the window still opens the
        // polaroids, and whoever gets both is on a screen where only one of them lands.
        match tapped {
            true => vec![Action::OpenMenu],
            false => Vec::new(),
        }
    }

    /// Which of the two the release means depends on whether the hold already fired. A
    /// press that never reached the threshold is a lock; one that did is a shutdown the
    /// user has been watching the screen for.
    fn power_up(&mut self) -> Vec<Action> {
        let held = self.power_hold_fired;
        self.power_down_at = None;
        self.power_hold_fired = false;
        vec![if held {
            Action::PowerOff
        } else {
            Action::PowerTap
        }]
    }

    /// The press always lands. Volume is held for repeat, so putting a chord window in front
    /// of every press would make the whole control feel slow to serve a gesture used once a
    /// session; the pair is recognised behind its own presses and whoever acts on it undoes
    /// them.
    fn volume_press(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        let (mine, other, action) = match b {
            Btn::VolUp => (&mut self.vol_up_at, self.vol_down_at, Action::VolumeUp),
            _ => (&mut self.vol_down_at, self.vol_up_at, Action::VolumeDown),
        };
        *mine = Some(now);
        match b {
            Btn::VolUp => self.vol_up_ramp = None,
            _ => self.vol_down_ramp = None,
        }
        let mut out = vec![action];
        let paired = other.is_some_and(|t| now.abs_diff(t) <= MUTE_CHORD_MS);
        if paired && !self.mute_fired {
            self.mute_fired = true;
            out.push(Action::MuteToggle);
        }
        out
    }

    fn volume_release(&mut self, b: Btn) -> Vec<Action> {
        match b {
            Btn::VolUp => (self.vol_up_at, self.vol_up_ramp) = (None, None),
            _ => (self.vol_down_at, self.vol_down_ramp) = (None, None),
        }
        if self.vol_up_at.is_none() && self.vol_down_at.is_none() {
            self.mute_fired = false;
        }
        Vec::new()
    }

    fn rewind_start(&mut self) -> Vec<Action> {
        if self.rewinding {
            return Vec::new();
        }
        let mut out = Vec::new();
        out.extend(self.ff_clear());
        self.rewinding = true;
        out.push(Action::RewindStart);
        out
    }

    fn rewind_stop(&mut self) -> Vec<Action> {
        if !self.rewinding {
            return Vec::new();
        }
        self.rewinding = false;
        vec![Action::RewindStop]
    }

    /// ★ 2026-10-20：**L2 的两种手势** —— 按住是倒带（一直如此），双击是慢放并把它锁住。
    ///
    /// 和 `ff_down` 那套状态机一模一样，因为要求一模一样：双击的第二下建立锁，它的松开不能
    /// 把锁解开；锁着的时候**再按一次就是解锁的那一按**，所以那一按既不倒带也不发动作，松开
    /// 时才发 `SlowStop`。区别只有两个，都是 L2 自己的：双击的第一下已经起了倒带，所以判定成
    /// 双击时先 `rewind_stop` —— 慢放和倒带是同一个时间的两个方向，不能同时在跑；而锁着的时候
    /// 按住 L2 **不倒带**，因为那一按是给锁的，不是给历史的。
    fn l2_down(&mut self, now: Millis) -> Vec<Action> {
        if self.slow_latched {
            self.slow_latching_press = false;
            return Vec::new();
        }
        let double = self
            .l2_last_release
            .is_some_and(|rel| now.saturating_sub(rel) <= SLOW_TAP_MS);
        if !double {
            return self.rewind_start();
        }
        self.slow_latched = true;
        self.slow_latching_press = true;
        let mut out = self.rewind_stop();
        out.push(Action::SlowStart);
        out
    }

    fn l2_up(&mut self, now: Millis) -> Vec<Action> {
        self.l2_last_release = Some(now);
        if self.slow_latching_press {
            // The press that established the latch: letting go of it is not a second gesture.
            self.slow_latching_press = false;
            return Vec::new();
        }
        let mut out = self.rewind_stop();
        if self.slow_latched {
            self.slow_latched = false;
            out.push(Action::SlowStop);
        }
        out
    }

    fn ff_down(&mut self, now: Millis) -> Vec<Action> {
        if self.rewinding {
            return Vec::new();
        }
        if self.ff_latched {
            // Any further press is the one whose release clears the latch.
            self.ff_latching_press = false;
        } else {
            let double = self
                .r2_last_release
                .is_some_and(|rel| now.saturating_sub(rel) <= FF_DOUBLE_TAP_MS);
            self.ff_latched = double;
            self.ff_latching_press = double;
        }
        if self.ff_on {
            return Vec::new();
        }
        self.ff_on = true;
        vec![Action::FfStart]
    }

    fn ff_up(&mut self, now: Millis) -> Vec<Action> {
        self.r2_last_release = Some(now);
        if self.ff_latching_press {
            self.ff_latching_press = false;
            return Vec::new();
        }
        self.ff_clear()
    }

    fn ff_clear(&mut self) -> Vec<Action> {
        self.ff_latched = false;
        self.ff_latching_press = false;
        if !self.ff_on {
            return Vec::new();
        }
        self.ff_on = false;
        vec![Action::FfStop]
    }
}

/// Which arm of `Gestures::game_shoulder` a shoulder key owns. Only the two that can be
/// handed to a game have one; anything else is zero, which is what makes the bit test in
/// `up` a no-op for every other button.
fn shoulder_bit(b: Btn) -> u16 {
    match b {
        Btn::L2 => 1,
        Btn::R2 => 2,
        _ => 0,
    }
}

fn chord(b: Btn) -> Option<(u16, Action)> {
    Some(match b {
        Btn::Up => (1, Action::BrightnessUp),
        Btn::Down => (2, Action::BrightnessDown),
        Btn::Left => (4, Action::BlueLightDown),
        Btn::Right => (8, Action::BlueLightUp),
        Btn::L1 => (16, Action::LoadState),
        Btn::R1 => (32, Action::SaveState),
        Btn::X => (64, Action::MaskCycle),
        Btn::Y => (128, Action::ColorCycle),
        Btn::A => (256, Action::CheatToggle),
        Btn::VolUp => (512, Action::AudioProfileNext),
        Btn::VolDown => (1024, Action::AudioProfilePrev),
        // ★★ **2026-10-21 修正：这条不许拆。** 先前把 SELECT+START 整条从表里摘掉，是误读了
        //   六六的意思 —— 他要的只是"**游戏里**把 START 还给游戏"，不是"货架上也不要切主题"。
        //   现在表里留着它，但**只在没有游戏时成立**：`Gestures::start_to_game` 由 `Session`
        //   每帧按"仓里有没有卡"置位，置上了 START 就连 chord 表都不查（见 `down` 的 `_` 臂）——
        //   货架照旧切日/夜主题，游戏里按住 SELECT 再按 START 是游戏自己的暂停/菜单键。
        Btn::Start => (2048, Action::ModeToggle),
        // ★ 2026-10-25：**`L2`/`R2` deliberately never enter this table.** They are not free
        //   — a held SELECT turns them into the transport, and that is done by the `L2|R2`
        //   arm falling through to `l2_down`/`ff_down` rather than by a chord entry. Putting
        //   them here would spend the press as a chord and lose the hold-to-rewind that
        //   `SELECT+L2` has always meant. On PS1 a **bare** press is the game's instead
        //   (`l2r2_to_game`), so the table must stay empty for these two either way.
        // **B is free.** It was a placeholder and is not any more: the shelf cycle
        // sat on SELECT+B and SELECT+L2 while it was being designed (it is a hold of Y now), and
        // SELECT+R2 was the second key to the overlay ring until `SELECT+X` became the one key
        // that means "the screen's own look" on every machine — on a Game Boy the two did exactly
        // the same thing, and a duplicate that only appears on one of the three is worse than no
        // key at all. A bare B, L2 or R2 is still the game's, chord key or not.
        _ => return None,
    })
}

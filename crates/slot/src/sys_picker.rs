//! 机种选择的时钟与状态：帘子满闸之后十格在哪一刻浮上来、光标走到哪、被拒绝时抖谁。
//! 不画东西、不碰卡 —— `App` 问它要一帧的读数，问它一次按键的结论。
//!
//! 与 `core_picker` 分开是刻意的：那个是「同一张卡带里的两个插座」，格子会跳、会有芯片
//! 在飞；这个是「帘子上的一张十格目录」，走的是网格。两套动作放进一个类型，读的人迟早
//! 会把"跳"当成"走"。
//!
//! **这块东西没有"关闭"**。它不是可以打开的浮层，而是卷闸满闸那一段里多出来的内容 ——
//! 帘子卷起来它就没了，收掉它的是转场（`App::settle_system_switch`），不是它自己。
//! 所以这里没有 `closing` / `finished`，只有一个"什么时候开始浮现"。

use slot_store::System;
use slot_ui::{Millis, Refusal, SYS_PICKER_COLS, SYS_PICKER_COUNT, SYS_PICKER_ROWS};

/// 帘子满闸之后，十格浮上来要多久。比帘子自己的任何一段都短 —— 它是"在已经停下来
/// 的帘子上显影"，不是第二段转场。
pub const REVEAL_MS: Millis = 160;

/// 一格长到手里那么大要多久。**180 ms**：比翻页慢一点，因为它不是"翻页"是"光落在
/// 这一格上"；再慢就拖手了。
pub const MOVE_MS: Millis = 180;

/// 一次按键的结论，交给 `App` 去执行。这里只说自己想要什么，不动货架、不动帘子。
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// 自己的事：光标走了、被拒绝了。帘子一动不动。
    Nothing,
    /// 换到这一台。`App` 会让帘子卷回去，卷起时露出的是新货架。
    Go(System),
    /// 不换了。`App` 同样让帘子卷回去 —— 但卷起时露出的还是原来那一台。
    Cancel,
}

#[derive(Copy, Clone)]
pub struct SysPicker {
    /// 手里那一格。
    cursor: usize,
    /// 上一格。移动的那 180 ms 里它的放大在退回去 —— 见 `slot_ui::sys_picker::hot_of`。
    prev: usize,
    /// 那一次移动开始的时刻。
    moved: Millis,
    /// 帘子盖满、这一块开始浮现的时刻。`None` = 帘子还在落，按钮还一个都不该出现。
    revealed: Option<Millis>,
    /// 按 A 选了一台没卡的机种时抖一下。抖的是那一格，不是整屏。
    refusal: Option<Refusal>,
}

impl SysPicker {
    /// 在 `cursor` 那一格上备好。`cursor` 是**当前货架**的机种在列表里的位置 —— 打开时
    /// 光标就停在"我现在在哪"上，所以按 B 回去连想都不用想。
    ///
    /// 这一刻**什么都不显示**：帘子还要 0.6 秒才落满，见 `reveal`。
    pub fn open(cursor: usize, now: Millis) -> Self {
        let cursor = cursor.min(SYS_PICKER_COUNT - 1);
        SysPicker {
            cursor,
            prev: cursor,
            // 落定状态：浮现的那一帧这一格就已经是大的，不从小的长出来 —— 它是"浮上来"，
            // 不是"光标长出来"。
            moved: now.saturating_sub(MOVE_MS),
            revealed: None,
            refusal: None,
        }
    }

    /// 帘子盖满了，可以显影。**只有第一次算**：后面每一帧都会问一遍，重复调不该把淡入
    /// 从头再来一次。
    pub fn reveal(&mut self, now: Millis) {
        self.revealed.get_or_insert(now);
    }

    /// 帘子盖满了没有。没盖满时一个像素都不画 —— 帘子还在落的过程中就看见按钮，
    /// 那读作"面板在帘子底下"。
    pub fn revealed(&self) -> bool {
        self.revealed.is_some()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn prev(&self) -> usize {
        self.prev
    }

    /// 浮现的不透明度：`REVEAL_MS` 淡入，之后恒为 1。帘子盖着的时候是 0。
    pub fn alpha(&self, now: Millis) -> f32 {
        match self.revealed {
            Some(at) => (now.saturating_sub(at) as f32 / REVEAL_MS as f32).clamp(0.0, 1.0),
            None => 0.0,
        }
    }

    /// 上一次移动走了多少。1.0 = 已经落定。
    pub fn moved(&self, now: Millis) -> f32 {
        (now.saturating_sub(self.moved) as f32 / MOVE_MS as f32).clamp(0.0, 1.0)
    }

    /// 这一帧要不要抖（选了一台没卡的机种）。
    pub fn shake(&self, now: Millis) -> f32 {
        self.refusal.map_or(0.0, |r| r.offset(now))
    }

    /// 方向键。走一格就重新起一次过渡的时钟。**帘子还没落满时不走** —— 那会儿按钮
    /// 还不在屏上，走了也看不见，反而在浮现的第一帧就露出一个已经到了的位置。
    pub fn move_by(&mut self, dx: i32, dy: i32, now: Millis) -> bool {
        if !self.revealed() {
            return false;
        }
        let to = slot_ui::sys_picker_step(self.cursor, dx, dy);
        if to == self.cursor {
            return false;
        }
        // 过渡中再按一下：从「上一次移动的落点」接着走，而不是从屏幕上看不见的中间态
        // 开始 —— 那会让连按时光标忽大忽小。
        self.prev = self.cursor;
        self.cursor = to;
        self.moved = now;
        self.refusal = None;
        true
    }

    /// A：选手里这一台。`has_carts` 是这一台在卡上有没有游戏 —— 没有就拒绝并抖一下，
    /// 帘子留着（用户可以接着走到旁边那一格，这才是他想要的）。
    pub fn choose(&mut self, target: System, has_carts: bool, now: Millis) -> Outcome {
        if !self.revealed() {
            return Outcome::Nothing;
        }
        if !has_carts {
            self.refusal = Some(Refusal::started(now));
            return Outcome::Nothing;
        }
        Outcome::Go(target)
    }

    /// B：不换了。帘子照旧卷回去，露出原来那一台。
    ///
    /// 不需要时间：它除了说"不换"之外没有任何状态要记 —— 收掉它、起回卷的都是 `App`。
    pub fn cancel(&mut self) -> Outcome {
        if !self.revealed() {
            return Outcome::Nothing;
        }
        Outcome::Cancel
    }
}

/// `i` 这一格在 5×2 网格里的行列，只给测试和调用方核对用。
pub fn grid_of(i: usize) -> (usize, usize) {
    (i % SYS_PICKER_COLS, (i / SYS_PICKER_COLS).min(SYS_PICKER_ROWS - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: Millis = 1_000;

    /// 帘子还在落的时候什么都不显示，一个像素都不。
    #[test]
    fn nothing_shows_until_the_shutter_is_down() {
        let p = SysPicker::open(3, T0);
        assert!(!p.revealed());
        assert_eq!(p.alpha(T0), 0.0);
        assert_eq!(p.alpha(T0 + 10_000), 0.0, "time alone does not reveal it");
    }

    /// 浮现：`REVEAL_MS` 淡入，之后恒满。
    #[test]
    fn it_fades_in_once_the_shutter_is_down() {
        let mut p = SysPicker::open(3, T0);
        p.reveal(T0);
        assert!(p.revealed());
        assert_eq!(p.alpha(T0), 0.0, "the fade starts at nothing");
        assert!(p.alpha(T0 + REVEAL_MS / 2) > 0.0);
        assert_eq!(p.alpha(T0 + REVEAL_MS), 1.0);
        assert_eq!(p.alpha(T0 + REVEAL_MS * 9), 1.0, "and stays up");
    }

    /// 反复 `reveal` 不该把淡入从头再来 —— 每一帧都会问一遍。
    #[test]
    fn revealing_twice_does_not_restart_the_fade() {
        let mut p = SysPicker::open(0, T0);
        p.reveal(T0);
        p.reveal(T0 + REVEAL_MS);
        assert_eq!(p.alpha(T0 + REVEAL_MS), 1.0);
    }

    /// 打开时光标停在原来那一台，而且**已经是大的**。
    #[test]
    fn it_opens_on_the_shelf_you_were_on() {
        let mut p = SysPicker::open(3, T0);
        p.reveal(T0);
        assert_eq!(p.cursor(), 3);
        assert_eq!(p.moved(T0), 1.0);
    }

    /// 帘子没落满时方向键不生效：按钮还不在屏上。
    #[test]
    fn the_arrows_wait_for_the_shutter() {
        let mut p = SysPicker::open(0, T0);
        assert!(!p.move_by(1, 0, T0));
        assert_eq!(p.cursor(), 0);
        p.reveal(T0);
        assert!(p.move_by(1, 0, T0));
        assert_eq!(p.cursor(), 1);
    }

    /// 走一格：旧格立刻开始收，180 ms 后落定。
    #[test]
    fn moving_hands_the_big_face_over() {
        let mut p = SysPicker::open(0, T0);
        p.reveal(T0);
        assert!(p.move_by(1, 0, T0));
        assert_eq!(p.prev(), 0);
        assert_eq!(p.moved(T0), 0.0);
        assert!(p.moved(T0 + MOVE_MS / 2) > 0.0);
        assert_eq!(p.moved(T0 + MOVE_MS), 1.0);
    }

    /// 走 0 步不算"动过" —— 否则每按一次都会白闪一下。
    #[test]
    fn a_step_that_goes_nowhere_is_still() {
        let mut p = SysPicker::open(0, T0);
        p.reveal(T0);
        assert!(!p.move_by(0, 0, T0));
        assert_eq!(p.prev(), p.cursor());
    }

    /// 没卡的机种选不了：拒绝、抖、帘子还停在那儿。
    #[test]
    fn an_empty_machine_is_refused_but_the_shutter_stays() {
        let mut p = SysPicker::open(0, T0);
        p.reveal(T0);
        assert_eq!(p.choose(System::Pce, false, T0), Outcome::Nothing);
        assert!(p.shake(T0) != 0.0, "a refusal has to be visible");
        assert!(p.revealed(), "the panel must stay up on a refusal");
        assert_eq!(p.cursor(), 0, "and the caret must not move");
    }

    /// 有卡的机种：说清楚换到哪一台，`App` 据此决定帘子卷起时露出谁。
    #[test]
    fn choosing_a_machine_asks_for_it() {
        let mut p = SysPicker::open(0, T0);
        p.reveal(T0);
        assert_eq!(p.choose(System::Pce, true, T0), Outcome::Go(System::Pce));
    }

    /// B：不换。
    #[test]
    fn cancelling_asks_for_nothing() {
        let mut p = SysPicker::open(7, T0);
        p.reveal(T0);
        assert_eq!(p.cancel(), Outcome::Cancel);
    }

    /// 帘子还没落满时 A 和 B 都不算数：那一刻用户看的是帘子下来，不是在选东西。
    #[test]
    fn presses_before_the_shutter_lands_are_ignored() {
        let mut p = SysPicker::open(0, T0);
        assert_eq!(p.choose(System::Pce, true, T0), Outcome::Nothing);
        assert_eq!(p.cancel(), Outcome::Nothing);
    }

    /// 光标下标与网格对得上：第 6 格是第二行第一列（0-based）。
    #[test]
    fn the_grid_is_five_by_two() {
        assert_eq!(SYS_PICKER_COUNT, SYS_PICKER_COLS * SYS_PICKER_ROWS);
        assert_eq!(grid_of(0), (0, 0));
        assert_eq!(grid_of(4), (4, 0));
        assert_eq!(grid_of(5), (0, 1));
        assert_eq!(grid_of(9), (4, 1));
    }
}

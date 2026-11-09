use crate::hud::Millis;

/// Long enough to read as a movement, short enough to be over before it looks like
/// something the player has to dismiss.
const REFUSAL_MS: Millis = 300;

const SHAKE_PX: f32 = 6.0;
const SHAKE_HZ: f32 = 14.0;

/// The whole of the error UI, per spec section 12: no words, just a shake. One curve for
/// every refusal, so a cart that will not seat and an action that will not happen are
/// answered the same way. What moves is the caller's: a cart on its way back out, or the
/// whole presented image when there is nothing else on screen to flinch.
#[derive(Copy, Clone)]
pub struct Refusal {
    started: Millis,
}

impl Refusal {
    pub fn started(now: Millis) -> Self {
        Refusal { started: now }
    }

    pub fn active(&self, now: Millis) -> bool {
        now.saturating_sub(self.started) < REFUSAL_MS
    }

    /// Horizontal pixels off centre, decaying to nothing. Cosine rather than sine, so the
    /// first frame is already at full throw: a flinch is a knock and everything after it is
    /// settling. Starting at zero would make the frame the action was refused on the one
    /// frame that did not move.
    pub fn offset(&self, now: Millis) -> f32 {
        let age = now.saturating_sub(self.started);
        if age >= REFUSAL_MS {
            return 0.0;
        }
        let decay = 1.0 - age as f32 / REFUSAL_MS as f32;
        let secs = age as f32 / 1000.0;
        (secs * SHAKE_HZ * std::f32::consts::TAU).cos() * SHAKE_PX * decay
    }

    /// ★★ 2026-10-27（六六）：「拒绝时让光瞬灭一下」（光效第 3 条）。
    ///
    /// 拒绝 = **没通电** —— 所以它不是"再加一层光"，而是让本来亮着的那块**断一下**：
    /// 插卡位的待机背板（`slot_back_*.png` 那条带 LOGO 的亮条）在 90 ms 里灭掉再回来。
    /// 一次拒绝只该有一条"视觉句子"，抖动说的是"这一下没过"，断电说的是"这里没接上"，
    /// 两者同一帧起、各自收 —— 抖动 300 ms，断电先结束。
    ///
    /// 返回 0..1 的**变暗量**（1 = 全灭），调用方拿它乘待机灯的 alpha。
    pub fn dip(&self, now: Millis) -> f32 {
        const DIP_MS: Millis = 90;
        let age = now.saturating_sub(self.started);
        if age >= DIP_MS {
            return 0.0;
        }
        let u = 1.0 - age as f32 / DIP_MS as f32;
        u * u
    }
}

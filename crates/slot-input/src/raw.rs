#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Btn {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    /// Never reach the core: the GBA has neither button. They exist so the switcher has
    /// buttons of its own for the undo and the delete.
    X,
    Y,
    L1,
    R1,
    L2,
    R2,
    Start,
    Select,
    Menu,
    VolUp,
    VolDown,
    Power,
    Lid,
    /// ★ 2026-11-03（六六）：按下左/右摇杆。**前端动作键**，不是游戏键，永远不进核心掩码。
    ///   先接成 L3=呼出亮度调节、R3=呼出蓝光调节，作识别验证用；有摇杆的机型后续再分配。
    ///   设备上的实际码是 **0x139 / 0x13c**（不是标准的 BTN_THUMBL/THUMBR，见
    ///   `slot::input::evdev::code_to_btn` 那段注释）。
    L3,
    R3,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RawEvent {
    Down(Btn),
    Up(Btn),
}

pub type Millis = u64;

pub trait InputSource {
    fn poll(&mut self, now: Millis) -> Vec<RawEvent>;
}

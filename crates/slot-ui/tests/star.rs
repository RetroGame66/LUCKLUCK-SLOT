//! 收藏夹星标的锚点 —— ★ 2026-10-27（六六）：「GB/GBC 按现在的位置，其它的按你建议的」。
//!
//! 背景：星标位置原来是「洋红框右上角」推出来的。贴图换成整张之后洋红框 ≈ 整张脸，那个角
//! 就是卡带自己的角，而星标**中心**压在那个角上 ⇒ MD / NES / SNES 有一半挂到卡带外面
//! （实测 -6.1 / -8.6 / -4.8 px）。现在这张表是写死的，不再跟素材走。
//!
//! 钉三条：
//! 1. **每个机种**在**中卡与侧卡两种缩放**下，20 px 的星标都整个落在卡带矩形里
//!    （侧卡是按 `side` 缩过的，锚点必须跟着矩形走 —— 这就是"分数"的意义）；
//! 2. **圆形贴图**（PS1 盘、ARCADE / NEOGEO 代币）还要落在**内切圆**里：矩形分数放到圆上
//!    会落到盘外的空气里，那正是改这张表的原因之一；
//! 3. GB / GBC **保持原值** —— 六六明确说这两台按现在的位置，动了就是改错。

use slot_store::System;
use slot_ui::{size_for, star_at};

/// 星标画出来 20 px 高，中心落在锚点上（见 `App::draw_fav_stars`）。
const STAR_HALF: f32 = 10.0;

const ALL: [System; 10] = [
    System::Gba,
    System::Gb,
    System::Gbc,
    System::Sfc,
    System::Fc,
    System::Neogeo,
    System::Arcade,
    System::Md,
    System::Ps1,
    System::Pce,
];

/// 圆形的三台：盘 / 代币，贴图是内切圆，四角是透明的。
const ROUND: [System; 3] = [System::Ps1, System::Arcade, System::Neogeo];

#[test]
fn the_star_lands_inside_the_cart_at_both_scales() {
    for system in ALL {
        let size = size_for(system);
        for (label, scale) in [("middle", size.middle), ("side", size.side)] {
            let (w, h) = (size.w as f32 * scale, size.h as f32 * scale);
            let (fx, fy) = star_at(system);
            let (cx, cy) = (fx * w, fy * h);
            assert!(
                cx - STAR_HALF >= -0.01 && cx + STAR_HALF <= w + 0.01,
                "{system:?} ({label}): the star is off the cart sideways — \
                 centre {cx} in 0..{w}"
            );
            assert!(
                cy - STAR_HALF >= -0.01 && cy + STAR_HALF <= h + 0.01,
                "{system:?} ({label}): the star hangs off the cart — centre {cy} in 0..{h}"
            );
        }
    }
}

/// 盘和代币是圆的：矩形分数放到圆上会落到盘外。星标必须整个在盘里。
#[test]
fn the_star_stays_on_the_disc_and_not_in_the_air_beside_it() {
    for system in ROUND {
        let size = size_for(system);
        for (label, scale) in [("middle", size.middle), ("side", size.side)] {
            let (w, h) = (size.w as f32 * scale, size.h as f32 * scale);
            let (fx, fy) = star_at(system);
            let (dx, dy) = (fx * w - w / 2.0, fy * h - h / 2.0);
            let radial = (dx * dx + dy * dy).sqrt();
            assert!(
                radial + STAR_HALF <= w / 2.0 + 0.01,
                "{system:?} ({label}): the star's centre is {radial} from the middle of a \
                 {} radius disc — it hangs off the rim",
                w / 2.0
            );
        }
    }
}

/// 六六 2026-10-27 22:5x：「**GB/GBC 按现在的位置**」—— 这两台是竖版卡，标签纸在卡面下
/// 三分之一，星标落在纸角上本来就对。取值就是原来从洋红框推出来的那两个。
#[test]
fn the_two_portrait_machines_keep_where_the_star_already_was() {
    assert_eq!(star_at(System::Gb), (0.873, 0.291));
    assert_eq!(star_at(System::Gbc), (0.863, 0.329));
}

/// 其余横向机种统一一格 —— 这就是"切机种时星标不许在卡上滑一段"。
#[test]
fn the_landscape_machines_share_one_anchor() {
    for system in [System::Gba, System::Sfc, System::Fc, System::Md, System::Pce] {
        assert_eq!(star_at(system), (0.90, 0.12), "{system:?} has its own anchor");
    }
}

use slot_store::{scan, System};
use slot_ui::{
    cart_face, cart_placeholder, clean_label, label_colour, label_panel, label_text,
    size_for, CART_H, CART_W, FACE_H, FACE_W, LABEL_H, LABEL_W, LABEL_X, LABEL_Y,
    OUT_H, OUT_W,
};
use tempfile::TempDir;

fn tmp_root() -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    for sub in ["Games", "Labels", "Saves", "States", "System"] {
        std::fs::create_dir(d.path().join(sub)).expect("create content dir");
    }
    d
}

fn write_rom(d: &TempDir, name: &str, title: &str) {
    let mut rom = vec![0u8; 0x100];
    rom[0xa0..0xa0 + title.len()].copy_from_slice(title.as_bytes());
    std::fs::write(d.path().join("Games").join(name), rom).expect("write rom");
}

fn write_label(d: &TempDir, name: &str, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 3]) {
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            rgb.extend_from_slice(&px(x, y));
        }
    }
    let f = std::fs::File::create(d.path().join("Labels").join(name)).expect("create label");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("png header")
        .write_image_data(&rgb)
        .expect("png data");
}

fn pixel(face: &slot_ui::CartFace, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * face.w + x) * 4) as usize;
    [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]]
}

/// The label sits inset in the shell, so a face pixel is only the label's business inside
/// this rect. Coordinates are relative to the label's own top left.
fn label_pixel(face: &slot_ui::CartFace, x: u32, y: u32) -> [u8; 3] {
    pixel(face, LABEL_X + x, LABEL_Y + y)
}

/// ★ 2026-10-28：**槽的位置从 `size_for()` 问，不抄 `LABEL_*` 那四个常量。**
///
/// 那四个常量是 GBA 那一版的参考尺寸（`CART_H = 135`），而卡带按机种换了尺寸之后
/// （`size_for(Gba).h = 143` ⇒ 面是 480×286），常量与真面已经对不上；照常量取样会取到
/// 槽外面的壳上 —— 这也正是这三条测试在此前一直红着的原因。
fn slot_of(system: System) -> (slot_ui::CartSize, (u32, u32, u32, u32)) {
    let size = size_for(system);
    (size, size.label_box())
}

/// 槽里的一点。
fn slot_pixel(face: &slot_ui::CartFace, slot: (u32, u32, u32, u32), x: u32, y: u32) -> [u8; 3] {
    pixel(face, slot.0 + x, slot.1 + y)
}

/// 一张坏 PNG **回落**到这块自己的色，而不是让卡变空白。
///
/// ★★★ 2026-11-03（六六）：`Labels/` 重新成为卡面的来源（见
/// `a_label_in_the_labels_folder_is_what_the_slot_shows`），于是"图读不出来"这条又成立了。
/// 断言只有一样：整块槽仍是 `label_colour` 那个色 —— 坏文件既不该被画上去，也不该把
/// 底下那块纸一起抹掉。
#[test]
fn a_malformed_label_falls_back_to_a_flat_slot() {
    let d = tmp_root();
    write_rom(&d, "Broken.gba", "BROKEN");
    std::fs::write(d.path().join("Labels/Broken.png"), b"not a png").unwrap();
    let cart = &scan(d.path()).unwrap()[0];
    let face = cart_face(cart);
    let (size, slot) = slot_of(System::Gba);
    let (_, _, lw, lh) = slot;
    // The face is rasterised at the internal resolution, not at the size it is drawn: the row
    // scales its selection up, and a texture that is only ever downsampled stays sharp.
    assert_eq!((face.w, face.h), (size.face_w(), size.face_h()));
    assert!(face.rgba.iter().any(|b| *b != 0), "face is blank");
    let bg = label_colour(&label_text(cart));
    for y in [0, lh / 2, lh - 1] {
        for x in [0, lw / 2, lw - 1] {
            assert_eq!(slot_pixel(&face, slot, x, y), bg, "at {x},{y}");
        }
    }
}

#[test]
fn label_colour_is_stable_across_calls() {
    assert_eq!(label_colour("POKEMON EMER"), label_colour("POKEMON EMER"));
    assert_ne!(label_colour("POKEMON EMER"), label_colour("ADVANCEWARS"));
}

/// ★★★ 2026-11-03（六六）：玩家反馈「Labels 改了很不方便」—— **`Labels/` 的图回到卡面**。
///
/// 这条吸收了 2026-10-28 的 `a_label_in_the_retired_folder_never_reaches_the_face`
/// （那一版把贴纸这条路整条拆了，量的是"槽里没有别的像素来源"）。现在量的是相反的一件事：
/// 玩家自己放进 `Labels/` 的那张图，就是槽里画的东西 —— 而且**整块槽**都是它
/// （`art::cover` 铺满，不留边）。纯色图缩放之后仍是同一个色，所以四点取样足够。
#[test]
fn a_label_in_the_labels_folder_is_what_the_slot_shows() {
    const PINK: [u8; 3] = [0xd0, 0x20, 0xa0];
    let d = tmp_root();
    write_rom(&d, "Labelled.gba", "LABELLED");
    write_label(&d, "Labelled.png", 64, 64, |_, _| PINK);
    let cart = &scan(d.path()).unwrap()[0];
    let face = cart_face(cart);
    let (size, slot) = slot_of(System::Gba);
    let (_, _, lw, lh) = slot;
    assert_eq!((face.w, face.h), (size.face_w(), size.face_h()));
    for y in [0, lh / 2, lh - 1] {
        for x in [0, lw / 2, lw - 1] {
            assert_eq!(slot_pixel(&face, slot, x, y), PINK, "at {x},{y}");
        }
    }
}

/// ★★ 2026-10-28（六六）：「3 去掉」—— 卡面不印游戏名。
///
/// 这条以前叫 `a_long_title_stays_inside_the_label`，量的是"最长的标题也不能溢出槽"。
/// 名字不上卡面之后，"排不下"这件事根本不成立，于是断言变成**整块槽是一个纯色** ——
/// 它顺手守住"没有人偷偷把字画回去"。
#[test]
fn the_label_slot_is_one_flat_colour_and_no_words() {
    for stem in [
        "Supercalifragilisticexpialidocious Anniversary Edition",
        "Super Mario Kart",
    ] {
        let d = tmp_root();
        std::fs::write(d.path().join(format!("Games/{stem}.gba")), [0u8; 8]).unwrap();
        let cart = &scan(d.path()).unwrap()[0];
        let face = cart_face(cart);
        let (_, slot) = slot_of(System::Gba);
        let (_, _, lw, lh) = slot;
        let bg = label_colour(&label_text(cart));
        for y in 0..lh {
            for x in 0..lw {
                assert_eq!(
                    slot_pixel(&face, slot, x, y),
                    bg,
                    "{stem}: 槽里出现了别的东西 at {x},{y}"
                );
            }
        }
    }
}

#[test]
fn the_cart_box_matches_the_traced_outline() {
    let ratio = CART_W as f32 / CART_H as f32;
    assert!(
        (ratio - 1.778).abs() < 0.02,
        "aspect {ratio:.3}, the svg is being stretched"
    );
    assert_eq!(
        CART_W * 3,
        OUT_W,
        "three carts no longer span the shelf exactly"
    );
}

/// The label is wide and low: thin plastic beside it, a broad moulded grip above. A
/// uniform border reads as a frame, and a narrow label reads as a coaster.
#[test]
fn the_label_is_wide_and_sits_low() {
    let (x0, y0, x1, y1) = label_panel(CART_W, CART_H);
    let (w, h) = (CART_W as f32, CART_H as f32);
    let side = x0 as f32 / w;
    let top = y0 as f32 / h;
    let bottom = 1.0 - y1 as f32 / h;
    let width = (x1 - x0) as f32 / w;

    // Wide, not an exact number. 0.82 was the value on the day this was written, and pinning
    // it meant editing the test every time the label was nudged by a percent.
    assert!(
        width > 0.78,
        "the label is only {:.0}% of the cart wide, that reads as a panel not a label",
        width * 100.0
    );
    assert!(
        top > 0.18,
        "only {:.0}% of grip band above the label",
        top * 100.0
    );
    assert!(
        top > side * 2.0,
        "top band {top:.2} against side margin {side:.2}: that is a uniform border"
    );
    assert!(
        top > bottom * 1.6,
        "the label is not sitting low, top {top:.2} bottom {bottom:.2}"
    );
}

/// ★ 2026-10-24（六六）：「**那几个默认的 svg 都解除绑定然后删掉**，目前已经是卡带贴图了
/// 也不怕透光了」。
///
/// 这条以前叫 `cart_faces_are_clipped_to_the_silhouette`：它断言左上角 alpha 为 0，
/// 因为那张手绘的 GBA 轮廓在那里有一个圆角。**那张 svg 已经不在了** —— 现在卡带的形状
/// 只有一个来源，就是卡上 `System/Carts/cart_<machine>.png` 的 alpha。
///
/// 所以这里断言的是新的诚实行为：**卡上没有那张贴图时，脸是一整块平板**（不编一个假轮廓
/// 出来），中央不透明、四角也不透明。有贴图时裁剪跟着贴图走，那条由 `cart_art` 的
/// 单元测试守着（`Art::mask` / `Art::silhouette`）。
#[test]
fn a_face_without_machine_art_is_a_plain_panel() {
    let d = tmp_root();
    write_rom(&d, "Emerald.gba", "EMERALD");
    let cart = &scan(d.path()).unwrap()[0];
    let f = cart_face(cart);
    let px = |x: u32, y: u32| f.rgba[((y * f.w + x) * 4 + 3) as usize];
    assert_eq!(
        px(2, 2),
        255,
        "没有贴图时不该凭空画出一个圆角：整块都是不透明的"
    );
    assert!(px(f.w / 2, f.h / 2) > 250);
    assert_eq!(px(f.w - 3, f.h - 3), 255, "四角一致，没有假轮廓");
}

/// Every case here is a real filename from the development card.
#[test]
fn the_label_is_the_filename_without_its_tags() {
    let cases = [
        (
            "Pokemon - Emerald Version (USA, Europe)",
            "Pokemon Emerald Version",
        ),
        (
            "Pokemon - LeafGreen Version (USA, Europe) (Rev 1)",
            "Pokemon LeafGreen Version",
        ),
        ("Shrek (USA) (Rev 6)", "Shrek"),
        ("Metroid Fusion", "Metroid Fusion"),
        ("Button Test", "Button Test"),
        ("Pokemon - Corrupt", "Pokemon Corrupt"),
        ("Some Game [!]", "Some Game"),
        ("Spaced   Out  (USA)", "Spaced Out"),
    ];
    for (stem, want) in cases {
        assert_eq!(clean_label(stem), want, "for {stem}");
    }
}

/// A hyphen inside a word is part of the word.
#[test]
fn an_unspaced_hyphen_survives() {
    assert_eq!(clean_label("Spider-Man 2 (USA)"), "Spider-Man 2");
    assert_eq!(
        clean_label("Wario Land 4 - Time Attack"),
        "Wario Land 4 Time Attack"
    );
}

#[test]
fn a_name_that_is_all_tags_falls_back_rather_than_going_blank() {
    assert_eq!(clean_label("(USA) (Rev 1)"), "(USA) (Rev 1)");
    assert_eq!(clean_label(""), "");
}

/// The twelve character header title is what this task exists to stop using.
#[test]
fn the_header_title_no_longer_reaches_the_label() {
    let d = tmp_root();
    write_rom(
        &d,
        "Pokemon - Emerald Version (USA, Europe).gba",
        "POKEMON EMER",
    );
    let cart = &scan(d.path()).unwrap()[0];
    assert_eq!(label_text(cart), "Pokemon Emerald Version");
}

#[test]
fn two_regions_of_one_game_get_the_same_generated_colour() {
    let a = label_colour(&clean_label("Pokemon - Ruby Version (USA, Europe) (Rev 2)"));
    let b = label_colour(&clean_label("Pokemon - Ruby Version (Japan)"));
    assert_eq!(a, b, "the same game came out two colours");
}

#[test]
fn a_rom_with_no_header_title_is_labelled_from_its_stem() {
    let d = tmp_root();
    std::fs::write(d.path().join("Games/Homebrew Demo.gba"), [0u8; 8]).unwrap();
    let face = cart_face(&scan(d.path()).unwrap()[0]);
    let bg = label_colour("Homebrew Demo");
    assert!(
        (0..LABEL_H * LABEL_W).any(|i| label_pixel(&face, i % LABEL_W, i / LABEL_W) != bg),
        "an untitled rom got a blank label"
    );
}


/// Every machine's label panel has to fit inside that machine's own face.
///
/// The panel is stored per mille of the face and read back as pixels, and the two are one
/// rounding apart — but `label_panel`'s last two numbers are the far **corner**, and they were
/// handed to `paste_label` as a width and a height. The Advance cart pasted a 437 x 233 label
/// onto a 480 x 270 face and the last 24 rows went past the end of it: `index out of bounds:
/// the len is 518400 but the index is 518572`, at `cart.rs:405`. On the device that runs before
/// the first frame, on every boot, on any card with a game on it — so the launcher restarted
/// the frontend every three seconds and the handheld never left its boot splash. This is the
/// assertion that would have caught it, and `CartSize::label_box` is the name that makes the
/// two meanings hard to confuse again.
#[test]
fn every_label_panel_fits_inside_its_own_face() {
    for system in [System::Gba, System::Gb, System::Gbc] {
        let size = size_for(system);
        let (x, y, w, h) = size.label_box();
        assert!(w > 0 && h > 0, "{system:?}: the label panel is empty");
        assert!(
            x + w <= size.face_w(),
            "{system:?}: the label runs {} px past the right edge",
            x + w - size.face_w()
        );
        assert!(
            y + h <= size.face_h(),
            "{system:?}: the label runs {} px past the bottom edge",
            y + h - size.face_h()
        );
    }
}

/// And the face a real cart produces is its machine's face, whole. Nothing but a rom on a card
/// is needed to fail this one, which is what the panic above needed too.
#[test]
fn a_cart_face_comes_out_the_size_of_its_machines_face() {
    let d = tmp_root();
    write_rom(&d, "Emerald.gba", "POKEMON EMER");
    write_rom(&d, "Link.gbc", "LINKAWAKEN");
    write_rom(&d, "Tetris.gb", "TETRIS");
    for cart in scan(d.path()).unwrap() {
        let size = size_for(cart.system());
        let face = cart_face(&cart);
        assert_eq!(
            (face.w, face.h),
            (size.face_w(), size.face_h()),
            "{:?} came out the wrong size",
            cart.system()
        );
        assert_eq!(face.rgba.len(), (face.w * face.h * 4) as usize);
    }
}

/// The stand-in is drawn for every cart whose own face has not been built yet — on a card of
/// nine hundred games that is nearly all of them, every time the caret moves. It fills the
/// label recess, so it reads the same panel the cart above does.
#[test]
fn the_placeholder_fits_its_face_too() {
    for system in [System::Gba, System::Gb, System::Gbc] {
        let size = size_for(system);
        let f = cart_placeholder(size, system);
        assert_eq!((f.w, f.h), (size.face_w(), size.face_h()), "{system:?}");
        assert_eq!(f.rgba.len(), (f.w * f.h * 4) as usize, "{system:?}");
    }
}

/// ★ 2026-10-23 六六：货架尺寸按朝向分两档，屏上尺寸是硬约束。23:1x 起排列是
/// **零间距紧贴**（「两侧同一脚线然后间隔对称」）：圆心距 = (中宽 + 侧宽) / 2 ⇒ 侧卡内缘
/// 贴中卡外缘，左右严格对称，脚线同一（foot_y 与 offset 无关）。21:4x「露一半 / 外缘贴屏边」
/// 的间距已取消。
///
/// 横版：中间 360 宽、两侧 240 宽、圆心距 300 ⇒ 两侧各出屏 60（侧宽的四分之一）；竖版：
/// 中间 300 高、两侧 200 高（同样的 2:3 侧中比），圆心距 ≈ 217.39 ⇒ 三张全框。
#[test]
fn the_shelf_sizes_follow_the_orientation_rule() {
    // ★ 2026-10-23 23:5x（六六）：「主卡带居中画面，脚线根据它来逆推；两侧候选卡带必须以
    //   对称的形式（横版露一半在画面内，竖版取中间卡带对应侧与屏幕边的中点）」。
    //
    //   这一版把 23:1x 的「零间距紧贴」**废掉** —— 那是他说的「残留之前取消的排列方式」。
    //   两条尺子，各钉各的。
    const HALF_W: f32 = OUT_W as f32 / 2.0;
    const HALF_H: f32 = OUT_H as f32 / 2.0;

    let land = size_for(System::Gba);
    assert_eq!(land.w as f32 * land.middle, 360.0, "landscape middle width");
    assert_eq!(land.w as f32 * land.side, 240.0, "landscape side width");

    // ① **主卡居中**：水平居中，垂直也居中 —— 所以脚线就是 半屏 + 主卡半高。
    for system in [
        System::Gba,
        System::Gb,
        System::Gbc,
        System::Sfc,
        System::Fc,
        System::Neogeo,
        System::Arcade,
    ] {
        let c = size_for(system);
        let mid_w = c.w as f32 * c.middle;
        let mid_h = c.h as f32 * c.middle;
        assert!(
            (c.foot_y() - (HALF_H + mid_h / 2.0)).abs() < 1e-3,
            "{system:?}: the foot is derived from the centred main cart ({} vs {})",
            c.foot_y(),
            HALF_H + mid_h / 2.0
        );
        // 主卡的水平中点是 360，这条谁都不许动。
        assert!(mid_w <= OUT_W as f32, "{system:?} the main cart fits across");
    }

    // ② **横版：侧卡露一半在画面内** ⇒ 侧卡中心正好落在屏幕边上。
    let half_side = land.w as f32 * land.side / 2.0;
    let side_centre = HALF_W + land.pitch; // 右卡中心
    assert!(
        (side_centre - OUT_W as f32).abs() < 1e-3,
        "the right neighbour is centred on the screen edge, so half of it shows: {side_centre}"
    );
    let left_centre = HALF_W - land.pitch;
    assert!((left_centre - 0.0).abs() < 1e-3, "and the left mirrors it");
    // 看得见的那半 = 120；出屏的那半也是 120。
    let visible = OUT_W as f32 - (side_centre - half_side);
    assert!(
        (visible - 120.0).abs() < 1e-3,
        "half of a 240 wide neighbour is on screen: {visible}"
    );
    // 侧卡与主卡之间因此有一条缝 —— 不是零间距。（主卡右缘 540，侧卡左缘 600。）
    let mid_right = HALF_W + land.w as f32 * land.middle / 2.0;
    let side_left = side_centre - half_side;
    let gap = side_left - mid_right;
    assert!((gap - 60.0).abs() < 1e-3, "landscape gap is 60: {gap}");

    // ★ 2026-10-24：SFC / FC 各自一个高度。★ **2026-10-27 23:1x 改规矩**：六六
    //   「我给的都是不需要重缩放的，按一比一像素就行了」⇒ 高度不再从 alpha 比例推，而是
    //   贴图高 / 1.5：SFC 贴图 360×266 → 177（中卡 360×265.5）；FC 贴图 360×229 → 153
    //   （中卡 360×229.5）。宽度规则、缩放、圆心距仍与横版一致。
    for (system, h) in [
        (System::Sfc, 177u32),
        (System::Fc, 153),
    ] {
        let c = size_for(system);
        assert_eq!((c.w, c.h), (land.w, h), "{system:?} box");
        assert_eq!(
            (c.pitch, c.middle, c.side),
            (land.pitch, land.middle, land.side),
            "{system:?} scales"
        );
    }

    // ★★ 2026-10-27（六六）：**ARCADE / NEOGEO 的载体是代币**，不是卡带 ——
    //   `66SLOT_DISC_ARCADE/NEOGEO.png` 都是 300×300 的圆牌，所以它们跟 PS1 一样走正方形：
    //   中卡 300×300、侧卡 200×200、圆心距 300。原来借 GBA 的 240×135 会把圆牌拉扁。
    for system in [
        System::Neogeo,
        System::Arcade,
        System::Ps1,
    ] {
        let c = size_for(system);
        assert_eq!((c.w, c.h), (200, 200), "{system:?} is not a square box");
        assert!(
            (c.w as f32 * c.middle - 300.0).abs() < 1e-3
                && (c.h as f32 * c.middle - 300.0).abs() < 1e-3,
            "{system:?} middle is not 300×300"
        );
        assert!(
            (c.w as f32 * c.side - 200.0).abs() < 1e-3,
            "{system:?} side is not 200 wide"
        );
        assert!((c.pitch - 300.0).abs() < 1e-3, "{system:?} pitch is not 300");
    }

    // ③ **竖版：侧卡中心取「主卡侧缘」与「屏幕边」的中点。**
    //
    // ★ 2026-10-27 23:1x：中卡改成**贴图原尺寸**（GB/GBC 贴图 300×340 ⇒ 中卡 300×340.5），
    //   侧卡仍按"200 高"（`side = 200/227`）。于是主卡比原来宽了 39 px，而侧卡大小没变 ——
    //   **间距从 27.8 px 收到 16.9 px**，这是 1:1 的直接后果，不是算错。要保住 27.8 就得把
    //   侧卡缩到 175 高，那是另一条命令（六六 23:1x 没要）。
    for system in [System::Gb, System::Gbc] {
        let p = size_for(system);
        let mid_w = p.w as f32 * p.middle;
        let mid_h = p.h as f32 * p.middle;
        let side_w = p.w as f32 * p.side;
        let side_h = p.h as f32 * p.side;
        assert!(
            (mid_h - 340.5).abs() < 1e-2,
            "{system:?} middle height is {mid_h}, not the art's 340"
        );
        assert!((side_h - 200.0).abs() < 1e-2, "{system:?} side height is {side_h}");

        let mid_right = HALF_W + mid_w / 2.0;
        let wanted = (mid_right + OUT_W as f32) / 2.0; // 右卡中心的中点名
        let actual = HALF_W + p.pitch;
        assert!(
            (actual - wanted).abs() < 1e-3,
            "{system:?}: the right neighbour is centred on the midpoint between the main \
             cart's right edge and the screen edge ({actual} vs {wanted})"
        );
        // 左卡是同一条弧的镜像。
        assert!(
            ((HALF_W - p.pitch) - (OUT_W as f32 - wanted)).abs() < 1e-3,
            "{system:?}: the left neighbour mirrors the right"
        );
        // 三张全框。
        let left = HALF_W - p.pitch - side_w / 2.0;
        let right = HALF_W + p.pitch + side_w / 2.0;
        assert!(left > 0.0, "{system:?} left side not in frame: {left}");
        assert!(right < OUT_W as f32, "{system:?} right side not in frame: {right}");
        // 与主卡之间留一点缝，而不是贴在它身上。（主卡右缘 510，侧卡左缘 526.9。）
        let gap = (actual - side_w / 2.0) - mid_right;
        assert!(
            (gap - 16.9).abs() < 0.1,
            "{system:?} the portrait gap is {gap}, not 16.9"
        );
    }
}

/// 磁带与碟片**不是卡带行**，六六那条「主卡居中」的规矩不管它们：这两行的脚线是各自
/// 调出来的老值，`foot_y` 换算法之后必须靠 `drop` 反推回去，一个像素都不许漂。
///
/// ★ 这个测试是补的 —— 2026-10-23 23:5x 改 `foot_y` 时，`tape::SIZE` 的 drop 写成了
/// `-10`（应 `-32`）、`cd::SIZE` 写成 `24`（应 `-30`），两处注释还都写着「脚线仍是原值」。
/// 当时 `size_for` 的七个机种都有断言，正好**绕开了这两行**，所以错误一路活到出图。
/// 现在把这两个数字钉住。
#[test]
fn the_tape_and_disc_rows_keep_their_own_foot_lines() {
    // 磁带：底座 240 + 135*1.8/2 = 361.5，减 32 ⇒ 329.5（原值）。
    let tape = slot_ui::tape::SIZE;
    assert!(
        (tape.foot_y() - 329.5).abs() < 1e-2,
        "the tape foot is {} not 329.5 — check tape::SIZE's drop",
        tape.foot_y()
    );
    // 碟片：底座 240 + 260*1.4/2 = 422，减 30 ⇒ 392（原值）。
    let cd = slot_ui::cd::SIZE;
    assert!(
        (cd.foot_y() - 392.0).abs() < 1e-2,
        "the disc foot is {} not 392 — check cd::SIZE's drop",
        cd.foot_y()
    );
    // 两行都**不许**被当成卡带行去居中：它们的脚线不等于「半屏 + 主卡半高」。
    for size in [tape, cd] {
        let centred = OUT_H as f32 / 2.0 + size.h as f32 * size.middle / 2.0;
        assert!(
            (size.foot_y() - centred).abs() > 1.0,
            "this row has been pulled onto the centring rule by accident"
        );
    }
}

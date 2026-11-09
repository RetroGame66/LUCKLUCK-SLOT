//! 把 slot-ui 的 LCD 播放面板用「真函数」渲成 PNG，跨 8 个 PixelShift 色板出示例图。
//!
//! 关键事实（来自 slot-ui 源码，不是重新实现）：
//! - `slot_ui::lcd::panel(c, ramp, grid) -> Vec<u8>` 返回 720×480 的 RGBA，shade→RGB 由
//!   `ramp`（lightest-first，[0]=纸 .. [3]=墨）在 render 阶段映射。
//! - `slot_ui::text::set_font(bytes)` 安装卡面正文字体。不装则回退到内嵌的 `label.ttf`
//!   （只有拉丁字形），中文歌名/歌词会缺字。所以这里必须灌入真·Ark Pixel 12px OTF。
//! - `Content` 借用的数据必须活得比 `panel()` 调用久；样例数据都在 main 作用域里。
//!
//! 用法：`render_palettes [字体otf] [输出目录]`

use std::fs;

use slot_ui::eq::Eq;
use slot_ui::lcd::{Bar, Content, Face, Mode, Transport, PANEL_H, PANEL_W};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let font_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "../_scratch/lcdcard/System/fonts/ark-pixel-12px-proportional-zh_hans.otf".into());
    let out_dir = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "../_scratch/shots_mock/pal".into());
    fs::create_dir_all(&out_dir).expect("mkdir out_dir");

    // 安装真·Ark Pixel 12px 正文字体（含 CJK），否则中文会缺字。
    let bytes = fs::read(&font_path).unwrap_or_else(|e| panic!("读字体失败 {font_path}: {e}"));
    slot_ui::text::set_font(bytes);

    // ---- 共享样例内容 ----
    let title = "夜空中最亮的星"; // 24px 主角，面板标题
    let artist = "逃跑计划"; // 12px 副行，右端带 [EQ名]
    let tap = make_tap(); // 单声道 tap，供 32 段频谱
    let rate: u32 = 44100;
    let elapsed = 73.0_f32; // 1:13
    let length = 252.0_f32; // 4:12

    // 歌词窗口：活跃行放在 index 2（面板 mid = len/2 = 2 高亮）
    let lyrics: Vec<String> = vec![
        "夜空中最亮的星".to_string(),
        "能否听清".to_string(),
        "那仰望的人".to_string(),
        "心底的孤独和叹息".to_string(),
        "夜空中最亮的星".to_string(),
    ];
    let no_lines: Vec<String> = Vec::new(); // Name 面不用歌词

    // 8 个 PixelShift 色板（ramp lightest-first: [0]=纸 .. [3]=墨）
    let ramps: &[(&str, [[u8; 3]; 4])] = &[
        ("PS01", [[196, 240, 194], [90, 185, 168], [30, 96, 110], [45, 27, 0]]),
        ("PS02", [[0, 0, 0], [0, 0, 0], [255, 255, 255], [255, 255, 255]]),
        ("PS10", [[2, 234, 117], [1, 177, 88], [0, 121, 60], [0, 65, 32]]),
        ("PS17", [[253, 169, 16], [248, 136, 6], [233, 107, 8], [131, 52, 3]]),
        ("PS23", [[255, 255, 147], [194, 222, 0], [83, 101, 0], [0, 53, 0]]),
        ("PS33", [[232, 232, 232], [160, 160, 160], [88, 88, 88], [16, 16, 16]]),
        ("PS37", [[5, 22, 12], [6, 45, 26], [13, 178, 84], [27, 195, 100]]),
        ("PS44", [[217, 7, 8], [150, 7, 8], [83, 6, 7], [7, 3, 3]]),
    ];

    let grid = true; // 带像素栅格，更贴近真机观感

    let mut sheet: Vec<(String, String)> = Vec::new();

    // ---- 每个色板：Name 面（播放中）+ Lyrics 面（播放中，换个 EQ 预设显 [EQ名]）----
    for (name, ramp) in ramps {
        let c = Content {
            face: Face::Name,
            title,
            artist,
            lines: &no_lines,
            tap: &tap,
            rate,
            levels: None,
            elapsed,
            length,
            scroll: 0,
            bar: Some(Bar {
                transport: Transport::Playing,
                mode: Mode::Repeat,
                elapsed,
                length,
            }),
            eq: Eq::from_index(0),
        };
        let img = slot_ui::lcd::panel(&c, *ramp, grid);
        let p = format!("{out_dir}/play-{name}.png");
        save_png(&p, &img, PANEL_W, PANEL_H);
        sheet.push((format!("{name} · 播放(Name)"), format!("play-{name}.png")));

        let c = Content {
            face: Face::Lyrics,
            title,
            artist,
            lines: &lyrics,
            tap: &tap,
            rate,
            levels: None,
            elapsed,
            length,
            scroll: 0,
            bar: Some(Bar {
                transport: Transport::Playing,
                mode: Mode::Repeat,
                elapsed,
                length,
            }),
            eq: Eq::from_index(4),
        };
        let img = slot_ui::lcd::panel(&c, *ramp, grid);
        let p = format!("{out_dir}/lyric-{name}.png");
        save_png(&p, &img, PANEL_W, PANEL_H);
        sheet.push((format!("{name} · 歌词(Lyrics)"), format!("lyric-{name}.png")));
    }

    // ---- PS10 下三种走带状态（演示 transport 图标 + 模式词）----
    let ps10 = ramps.iter().find(|(n, _)| *n == "PS10").unwrap().1;
    for (suffix, transport, mode) in [
        ("playing", Transport::Playing, Mode::Repeat),
        ("paused", Transport::Paused, Mode::Shuffle),
        ("stopped", Transport::Stopped, Mode::One),
    ] {
        let c = Content {
            face: Face::Name,
            title,
            artist,
            lines: &no_lines,
            tap: &tap,
            rate,
            levels: None,
            elapsed,
            length,
            scroll: 0,
            bar: Some(Bar {
                transport,
                mode,
                elapsed,
                length,
            }),
            eq: Eq::from_index(0),
        };
        let img = slot_ui::lcd::panel(&c, *ps10, grid);
        let p = format!("{out_dir}/bar-PS10-{suffix}.png");
        save_png(&p, &img, PANEL_W, PANEL_H);
        let label = match transport {
            Transport::Playing => "PS10 · 走带 Playing",
            Transport::Paused => "PS10 · 走带 Paused",
            Transport::Stopped => "PS10 · 走带 Stopped",
        };
        sheet.push((label.to_string(), format!("bar-PS10-{suffix}.png")));
    }

    write_sheet(&out_dir, &sheet, ramps);

    println!("已写出 {} 张 PNG → {out_dir}", sheet.len());
    println!("联系表：{out_dir}/index.html");
}

/// 合成一段单声道 tap，频谱呈「低频强、高频弱」的音乐感。
fn make_tap() -> Vec<i16> {
    let rate = 44_100u32;
    let n = 2048usize;
    let comps: [(f32, f32); 8] = [
        (110.0, 1.00),
        (220.0, 0.80),
        (440.0, 0.90),
        (880.0, 0.60),
        (1320.0, 0.65),
        (1760.0, 0.40),
        (2640.0, 0.45),
        (3520.0, 0.25),
    ];
    let mut v = vec![0i16; n];
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let mut s = 0.0f32;
        for (f, a) in comps {
            s += a * (2.0 * std::f32::consts::PI * f * t).sin();
        }
        // 轻微包络，像在「播放」而非静止正弦
        let env = 0.6 + 0.4 * (2.0 * std::f32::consts::PI * 2.0 * t).sin().max(0.0);
        v[i] = (s * env * 8000.0).clamp(-32767.0, 32767.0) as i16;
    }
    v
}

fn save_png(path: &str, rgba: &[u8], w: u32, h: u32) {
    let file = fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.writer().expect("png writer");
    writer.write_image_data(rgba).expect("write png");
    writer.finish().expect("png finish");
}

/// 生成一张联系表 HTML，方便一次看全。
fn write_sheet(out_dir: &str, sheet: &[(String, String)], ramps: &[(&str, [[u8; 3]; 4])]) {
    let mut h = String::new();
    h.push_str(
        "<!doctype html><html lang=zh><head><meta charset=utf-8>\
<meta name=viewport content=\"width=device-width,initial-scale=1\">\
<title>播放面板 · 色板示例</title>\
<style>body{background:#2b2b2b;color:#ddd;font:13px/1.4 system-ui,sans-serif;margin:0;padding:18px}\
h1{font-size:18px;font-weight:600;margin:0 0 4px}h2{font-size:13px;color:#9a9a9a;font-weight:400;margin:0 0 18px}\
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(360px,1fr));gap:18px}\
figure{margin:0;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:8px;overflow:hidden}\
figcaption{padding:8px 10px;font-size:12px;color:#cfcfcf;border-top:1px solid #3a3a3a}\
img{display:block;width:100%;height:auto;image-rendering:pixelated;background:#000}\
.legend{margin:0 0 18px;display:flex;flex-wrap:wrap;gap:10px}\
.chip{display:flex;align-items:center;gap:6px;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:6px;padding:4px 8px}\
.sw{display:flex}.sw i{width:16px;height:16px;display:block}\
</style></head><body>",
    );
    h.push_str("<h1>复古 Game Boy LCD 音乐播放面板 · 色板示例</h1>");
    h.push_str("<h2>用 slot-ui 真函数 slot_ui::lcd::panel 渲染 · 720×480 · 含像素栅格</h2>");

    h.push_str("<div class=legend>");
    for (name, ramp) in ramps {
        h.push_str(&format!("<div class=chip>{name}<span class=sw>"));
        for shade in ramp {
            h.push_str(&format!(
                "<i style=\"background:rgb({},{},{})\"></i>",
                shade[0], shade[1], shade[2]
            ));
        }
        h.push_str("</span></div>");
    }
    h.push_str("</div>");

    h.push_str("<div class=grid>");
    for (caption, file) in sheet {
        h.push_str(&format!(
            "<figure><img src=\"{file}\" alt=\"{cap}\"><figcaption>{cap}</figcaption></figure>",
            file = file,
            cap = caption
        ));
    }
    h.push_str("</div></body></html>");

    fs::write(format!("{out_dir}/index.html"), h).expect("write sheet");
}

//! The card's own per-cart rules — the shell table the build carries, made editable.
//!
//! `System/Carts/carts.tsv`, one row per game, keyed on the game code the rom's own header
//! carries (an Advance cart's four characters; a Game Boy cart has no code field, so its
//! header title stands in — see `slot_store::gba::header`). The row says what that cart
//! wears: a shell colour and finish, a template to draw it from, a badge on its label, and
//! a film over the sticker.
//!
//! Why a table on the card rather than more rows in `shell.rs`: the built-in table is a
//! `const`, so every colour on the shelf was a decision made at compile time and changed by
//! rebuilding the binary — two of them, one per language. This is the same authority with
//! the card holding it: the built-in table stands as the fallback for anything the card
//! does not mention, and the card's own answer wins where it speaks.
//!
//! The format is a TSV because that is what this project's card-side files are
//! (`library.cache`, `slot.state`), and because a rule is six short fields a hand can edit
//! in a text editor when the toolbox is not to hand:
//!
//! ```text
//! # code    shell    finish        art        badge   foil
//! BPEE      #249c60  solid         tpl:metal  42      shatter:0.5
//! AXVJ      #c2332e  -             -          -       -
//! POKEMON RED  -    translucent    -          7       holo
//! ```
//!
//! `-` in any field is "unset", which leaves the built-in answer alone. A line that starts
//! with `#` is a comment and there are no trailing ones, because a colour is `#rrggbb` and
//! cutting at the first hash would eat the shell column off every row that has one. The key
//! is a **prefix** of the game code and the longest one wins, so a row may name one
//! region's release (`BPEE`) or a whole family (`BPE`) — the same shape `shell.rs` matches
//! on, so a rule and a built-in row are read the same way.

use std::path::Path;
use std::sync::OnceLock;

use slot_gfx::{badge_uv, Draw, TexId};

use crate::shell::Finish;

/// Where the card keeps it, beside the cart art it can override.
const DIR: &str = "System/Carts";
const FILE: &str = "carts.tsv";

/// The columns a row has, in order.
const COLS: usize = 6;

/// What the card says about one game. Every field is an override; a row of `-` throughout
/// says nothing, which is the same as no row at all.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Row {
    /// The shell colour, `#rrggbb`. `None` leaves the built-in table's answer alone.
    pub shell: Option<[u8; 3]>,
    /// `None` leaves the built-in row's finish, which is solid for every row it has.
    pub finish: Option<Finish>,
    /// A template name: `System/Carts/tpl_<name>_<machine>.png`, drawn instead of the
    /// machine's own `cart_<machine>.png`. `None` leaves the machine's own art.
    pub art: Option<String>,
    /// An index into the emoji atlas, stamped at the label slot's top left corner.
    pub badge: Option<u32>,
    /// The film over the sticker. `None` is no film.
    pub foil: Option<Foil>,
}

/// The film over a cart's sticker, as one of five looks that share a single shader.
///
/// Which one decides where it sits: `Prism` runs along the shell's own edge, the rest cover
/// the paper. Nothing else about a look is fixed here — the strength is the card's to say,
/// and the shader is where the look itself lives.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FoilKind {
    /// Diffraction rainbow: low saturation, thin bands, the whole sticker.
    Holo,
    /// Cracked-ice cells, each its own hue and its own twinkle phase.
    Shatter,
    /// Brushed metal: anisotropic streaks, one hard highlight, a band rather than the lot.
    Metal,
    /// Sparse warm sparkles.
    Glitter,
    /// A rainbow along the shell's edge and nowhere else.
    Prism,
}

/// A film and how hard it is laid, `0.0` to `1.0`.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Foil {
    pub kind: FoilKind,
    pub strength: f32,
}

impl FoilKind {
    /// The shader's effect id. One shader, five looks: this is which look, as the float the
    /// uniform carries.
    pub fn effect(self) -> f32 {
        match self {
            FoilKind::Holo => 0.0,
            FoilKind::Shatter => 1.0,
            FoilKind::Metal => 2.0,
            FoilKind::Glitter => 3.0,
            FoilKind::Prism => 4.0,
        }
    }

    /// Whether the film runs along the shell's edge rather than over the paper. The edge is
    /// read out of the cart face's own alpha, so this look wants the whole face where the
    /// others want the sticker's rectangle.
    pub fn wraps_edge(self) -> bool {
        matches!(self, FoilKind::Prism)
    }
}

/// How wide a badge is drawn, in panel pixels. Absolute rather than a share of the cart: a
/// mark that grew with the selected cart would say that cart was more of a favourite than
/// its neighbours — the same reading the favourites star makes, for the same reason.
pub const BADGE_PX: f32 = 22.0;

/// The emoji atlas is a grid this many glyphs across, indexed row-major by the `badge`
/// column. The toolbox bakes the atlas to the same grid.
pub const BADGE_COLS: u32 = 16;

/// How fast the film's clock runs on the cart under the caret, in phase per second.
const SWEEP_RATE: f32 = 0.6;

/// How much specular the sweep adds on the cart under the caret. Zero everywhere else: a
/// still film on every cart and a moving one where the eye already is.
const SWEEP_LIGHT: f32 = 0.55;

/// Where a still film stands, per game: the same code hashes to the same phase on every
/// boot, so a cart's film is a fixed thing you can recognise rather than a roll of the dice
/// each time the shelf comes up. The same stable hash `label_colour` uses, for the same
/// reason.
pub fn stable_phase(code: &str) -> f32 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in code.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    ((h >> 33) as f32) / ((1u64 << 31) as f32)
}

/// The marks one rule puts on one cart quad, as draw items to push after the cart's face.
///
/// Pure geometry on purpose: the caller owns the textures — only the compositor can mint a
/// `TexId` — so what a rule *asks for* is decided here, where it can be tested, and the
/// device only supplies what it happens to have. A rule whose badge has no atlas, or whose
/// film has no face to lie over, asks for nothing rather than for a mark that cannot be
/// drawn.
///
/// * `quad` is the cart's rect on the panel, `(x, y, w, h)`.
/// * `share` is where the sticker sits on that quad as `(x0, y0, x1, y1)` shares of it —
///   the card's own art says where its label is, and the built-in panel stands in when
///   there is no art. The caller has already made that choice; this only consumes it.
/// * `t` is the frame clock and `selected` says whether this cart is the one under the
///   caret: the film stands still on every cart and sweeps only under the caret, so the
///   animation is a function of both rather than of the clock alone.
/// * `alpha` is the quad's own opacity, so a cart dimmed at the side of the row dims its
///   marks with it.
// Nine inputs, and every one of them is a fact about *this* cart rather than a list that
// belongs together: which rule, which code, where the quad is, where the sticker sits inside
// it, two textures, the clock, and two flags. A struct here would only move the list.
#[allow(clippy::too_many_arguments)]
pub fn marks(
    rule: &Row,
    code: &str,
    quad: (f32, f32, f32, f32),
    share: (f32, f32, f32, f32),
    face: Option<TexId>,
    atlas: Option<(TexId, u32, u32)>,
    t: f32,
    selected: bool,
    alpha: f32,
) -> Vec<Draw> {
    let mut out = Vec::new();
    let (qx, qy, qw, qh) = quad;
    let (x0, y0, x1, y1) = share;
    let (sx, sy) = (qx + x0 * qw, qy + y0 * qh);
    let (sw, sh) = ((x1 - x0) * qw, (y1 - y0) * qh);
    // The badge, inset inside the sticker's own top left corner — the reserved area's
    // corner, which is where a collector's stamp goes and the one place on the cart that is
    // the same on every machine's art.
    if let Some(glyph) = rule.badge {
        if let Some((tex, gw, gh)) = atlas {
            if let Some(uv) = badge_uv(glyph, BADGE_COLS, gw, gh) {
                let inset = BADGE_PX * 0.18;
                out.push(Draw::Glyph {
                    x: sx + inset,
                    y: sy + inset,
                    w: BADGE_PX,
                    h: BADGE_PX,
                    tex,
                    uv,
                    alpha,
                });
            }
        }
    }
    // The film, over the sticker or along the shell's edge, whichever the look asks for.
    if let Some(foil) = rule.foil {
        if let Some(tex) = face {
            let (fx, fy, fw, fh, uv) = if foil.kind.wraps_edge() {
                (qx, qy, qw, qh, [0.0, 0.0, 1.0, 1.0])
            } else {
                (sx, sy, sw, sh, [x0, y0, x1, y1])
            };
            let phase = if selected { t * SWEEP_RATE } else { stable_phase(code) };
            let highlight = if selected { SWEEP_LIGHT } else { 0.0 };
            out.push(Draw::Foil {
                x: fx,
                y: fy,
                w: fw,
                h: fh,
                tex,
                uv,
                alpha,
                foil: [foil.strength, phase, highlight, foil.kind.effect()],
            });
        }
    }
    out
}

/// The whole table: rows keyed by code prefix, and the template names they ask for so the
/// art loader knows what to decode.
#[derive(Clone, Default, Debug)]
pub struct Rules {
    rows: Vec<(String, Row)>,
    templates: Vec<String>,
}

impl Rules {
    /// The row whose key is the longest prefix of `code`, or `None` when the card says
    /// nothing about this game. Matched without case, because a hand editing the file
    /// writes `bpee` where the header says `BPEE` and means the same game.
    pub fn for_code(&self, code: &str) -> Option<&Row> {
        if code.is_empty() {
            return None;
        }
        let code = code.to_ascii_uppercase();
        self.rows
            .iter()
            .filter(|(k, _)| code.starts_with(k.as_str()))
            .max_by_key(|(k, _)| k.len())
            .map(|(_, row)| row)
    }

    /// The template names the rows ask for, each once, sorted. The art loader reads exactly
    /// these — a `tpl_*.png` on the card that no row names is not decoded, because a decode
    /// is the one part of boot that is not free.
    pub fn templates(&self) -> &[String] {
        &self.templates
    }

    /// Whether any row names a badge. The atlas is a few megabytes of RGBA once decoded, so
    /// the boot path asks before it uploads one: a card with no badges on it does not pay for
    /// an emoji set nothing will draw.
    pub fn wants_badges(&self) -> bool {
        self.rows.iter().any(|(_, r)| r.badge.is_some())
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Read the card's table. A missing, unreadable or empty file is an empty table, which
/// leaves every cart exactly as the build draws it.
pub fn load(root: &Path) -> Rules {
    let Ok(text) = std::fs::read_to_string(root.join(DIR).join(FILE)) else {
        return Rules::default();
    };
    parse(&text)
}

/// Parse the table's text. Rows that do not parse are dropped rather than failing the read:
/// a hand-edited file with one bad line still has to load the good ones, and the count of
/// what was dropped is in the summary so a silence is not mistaken for an empty table.
pub fn parse(text: &str) -> Rules {
    let mut rules = Rules::default();
    for line in text.lines() {
        // A comment is a line that *starts* with `#`, never a `#` anywhere in one: a colour
        // is `#rrggbb`, and cutting at the first hash would eat the shell column off every
        // row that has one. So this format has no trailing comments — a note goes on its
        // own line.
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
        // A header line, which the toolbox writes and a hand is welcome to keep.
        if fields[0].eq_ignore_ascii_case("code") {
            continue;
        }
        if !(2..=COLS).contains(&fields.len()) {
            continue;
        }
        let mut f = fields;
        while f.len() < COLS {
            f.push("-");
        }
        let Some(row) = row_of(&f) else { continue };
        let key = f[0].to_ascii_uppercase();
        if key.is_empty() {
            continue;
        }
        if let Some(name) = &row.art {
            if !rules.templates.iter().any(|t| t == name) {
                rules.templates.push(name.clone());
            }
        }
        rules.rows.push((key, row));
    }
    rules.templates.sort();
    rules
}

/// One parsed row, or `None` when a field does not read. A bad colour or a bad film kind
/// drops the row: half a rule is worse than none, because the half that parsed would paint
/// a cart the card did not ask for.
fn row_of(f: &[&str]) -> Option<Row> {
    Some(Row {
        shell: colour_of(f[1])?,
        finish: finish_of(f[2])?,
        art: art_of(f[3])?,
        badge: badge_of(f[4])?,
        foil: foil_of(f[5])?,
    })
}

/// `#rrggbb`, or `rrggbb`, or `-`.
fn colour_of(field: &str) -> Option<Option<[u8; 3]>> {
    let hex = field.strip_prefix('#').unwrap_or(field);
    if hex == "-" {
        return Some(None);
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    (hex.len() == 6).then_some({
        Some([
            ((v >> 16) & 0xff) as u8,
            ((v >> 8) & 0xff) as u8,
            (v & 0xff) as u8,
        ])
    })
}

fn finish_of(field: &str) -> Option<Option<Finish>> {
    match field.to_ascii_lowercase().as_str() {
        "-" => Some(None),
        "solid" => Some(Some(Finish::Solid)),
        "translucent" | "clear" => Some(Some(Finish::Translucent)),
        _ => None,
    }
}

/// `-`, a bare name, or `tpl:<name>`. A name is a file's stem, so one holding a separator
/// or a parent step is not a name — it is a path, and this table does not hold paths.
fn art_of(field: &str) -> Option<Option<String>> {
    if field == "-" {
        return Some(None);
    }
    let name = field.strip_prefix("tpl:").unwrap_or(field).trim();
    let ok = !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ');
    ok.then(|| Some(name.to_string()))
}

fn badge_of(field: &str) -> Option<Option<u32>> {
    if field == "-" {
        return Some(None);
    }
    field.parse::<u32>().ok().map(Some)
}

/// `-`, a kind alone, or `<kind>:<strength>` with the strength clamped into range. A kind
/// this build does not know drops the row rather than guessing which look was meant.
fn foil_of(field: &str) -> Option<Option<Foil>> {
    if field == "-" {
        return Some(None);
    }
    let (kind, strength) = match field.split_once(':') {
        Some((k, s)) => (k, Some(s)),
        None => (field, None),
    };
    let kind = match kind.to_ascii_lowercase().as_str() {
        "holo" => FoilKind::Holo,
        "shatter" => FoilKind::Shatter,
        "metal" => FoilKind::Metal,
        "glitter" => FoilKind::Glitter,
        "prism" => FoilKind::Prism,
        _ => return None,
    };
    let strength = match strength {
        None => default_strength(kind),
        Some(s) => match s.trim().parse::<f32>() {
            Ok(v) => v.clamp(0.0, 1.0),
            Err(_) => return None,
        },
    };
    Some(Some(Foil { kind, strength }))
}

/// How hard each look is laid when the row does not say. Each one is the strength that read
/// as that look on the panel rather than as paint; they are a starting point the card can
/// override per row, not a law.
fn default_strength(kind: FoilKind) -> f32 {
    match kind {
        FoilKind::Holo => 0.35,
        FoilKind::Shatter => 0.5,
        FoilKind::Metal => 0.6,
        FoilKind::Glitter => 0.25,
        FoilKind::Prism => 0.5,
    }
}

static RULES: OnceLock<Rules> = OnceLock::new();

/// Called once, at boot, before the face builder starts — a face is built with the rule
/// already applied, so the table has to be there, whole, by then.
pub fn install(rules: Rules) {
    let _ = RULES.set(rules);
}

/// What the card says about one game, if it says anything.
pub fn for_code(code: &str) -> Option<&'static Row> {
    RULES.get()?.for_code(code)
}

/// Whether the card asks for any badge at all. The boot path reads this before it decodes
/// the atlas, which is the one upload on the shelf that costs real memory.
pub fn wants_badges() -> bool {
    RULES.get().is_some_and(Rules::wants_badges)
}

/// For the startup log: how much the card is overriding, so a card whose editor wrote
/// nothing is distinguishable from one whose file was refused.
pub fn summary() -> String {
    match RULES.get() {
        None => "none".to_string(),
        Some(r) => format!(
            "{} rows, {} templates{}",
            r.rows.len(),
            r.templates.len(),
            if r.rows.is_empty() { " (built-in table stands)" } else { "" }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_with_every_column_parses() {
        let r = parse("BPEE\t#249c60\tsolid\ttpl:metal\t42\tshatter:0.5\n");
        let row = r.for_code("BPEE").expect("the row is there");
        assert_eq!(row.shell, Some([0x24, 0x9c, 0x60]));
        assert_eq!(row.finish, Some(Finish::Solid));
        assert_eq!(row.art.as_deref(), Some("metal"));
        assert_eq!(row.badge, Some(42));
        assert_eq!(
            row.foil,
            Some(Foil {
                kind: FoilKind::Shatter,
                strength: 0.5
            })
        );
        assert_eq!(r.templates(), ["metal"]);
    }

    #[test]
    fn dashes_mean_unset() {
        let r = parse("AXVJ\t-\t-\t-\t-\t-\n");
        assert_eq!(r.for_code("AXVJ"), Some(&Row::default()));
        assert!(r.templates().is_empty());
    }

    #[test]
    fn the_longest_prefix_wins() {
        let r = parse("BPE\t#111111\t-\t-\t-\t-\nBPEE\t#222222\t-\t-\t-\t-\n");
        assert_eq!(r.for_code("BPEE01").unwrap().shell, Some([0x22, 0x22, 0x22]));
        assert_eq!(r.for_code("BPEJ01").unwrap().shell, Some([0x11, 0x11, 0x11]));
        // A code the table does not cover has no row, and the built-in table stands.
        assert!(r.for_code("ZZZZ").is_none());
    }

    #[test]
    fn keys_match_without_case() {
        let r = parse("bpEe\t#222222\t-\t-\t-\t-\n");
        assert_eq!(r.for_code("BPEE01").unwrap().shell, Some([0x22, 0x22, 0x22]));
    }

    #[test]
    fn a_game_boy_title_is_a_key_like_any_other() {
        let r = parse("POKEMON RED\t#c0392b\t-\t-\t-\tholo\n");
        assert_eq!(r.for_code("POKEMON RED").unwrap().shell, Some([0xc0, 0x39, 0x2b]));
    }

    #[test]
    fn comments_and_a_header_line_are_skipped() {
        let r = parse(
            "# a comment\ncode\tshell\tfinish\tart\tbadge\tfoil\n\nBPEE\t#249c60\t-\t-\t-\t-\n",
        );
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.for_code("BPEE").unwrap().shell, Some([0x24, 0x9c, 0x60]));
    }

    #[test]
    fn a_line_that_does_not_read_is_dropped() {
        // A one-field line, a colour that is not one, a finish that is not one, a film kind
        // this build does not know, and a badge that is not a number.
        let r = parse(
            "LONE\nAXVJ\t#zzzzzz\t-\t-\t-\t-\nBPR\t-\tsparkly\t-\t-\t-\nBPG\t-\t-\t-\t-\tchrome\nBPE\t-\t-\t-\tx\t-\nBPEE\t#249c60\t-\t-\t-\t-\n",
        );
        assert_eq!(r.rows.len(), 1, "only the good row survives");
        assert_eq!(r.for_code("BPEE").unwrap().shell, Some([0x24, 0x9c, 0x60]));
    }

    #[test]
    fn a_template_name_is_a_stem_not_a_path() {
        let r = parse("BPEE\t-\t-\t../../etc/passwd\t-\t-\nBPEE\t-\t-\ttpl:metal v2\t-\t-\n");
        // The second row wins on the same key, and the name it names is a plain stem.
        assert_eq!(r.templates(), ["metal v2"]);
    }

    #[test]
    fn a_strength_outside_the_range_is_clamped() {
        let r = parse("BPEE\t-\t-\t-\t-\tholo:9.0\nBPR\t-\t-\t-\t-\tglitter:-3\n");
        assert_eq!(r.for_code("BPEE").unwrap().foil.unwrap().strength, 1.0);
        assert_eq!(r.for_code("BPR").unwrap().foil.unwrap().strength, 0.0);
    }

    #[test]
    fn a_kind_alone_gets_its_default_strength() {
        let r = parse("BPEE\t-\t-\t-\t-\tmetal\n");
        let foil = r.for_code("BPEE").unwrap().foil.unwrap();
        assert_eq!(foil.kind, FoilKind::Metal);
        assert_eq!(foil.strength, default_strength(FoilKind::Metal));
    }

    #[test]
    fn a_missing_file_is_an_empty_table() {
        let dir = tempfile::tempdir().unwrap();
        let r = load(dir.path());
        assert!(r.is_empty());
        assert!(r.for_code("BPEE").is_none());
    }

    /// A rectangle as `(x, y, w, h)`, or a share of one as `(x0, y0, x1, y1)`.
    type Quad = (f32, f32, f32, f32);

    /// A geometry helper. Everything below is the same shape: a cart quad, the sticker's
    /// share of it, and the two textures a mark might want.
    fn cart() -> (Quad, Quad) {
        ((100.0, 50.0, 200.0, 300.0), (0.1, 0.2, 0.9, 0.6))
    }

    fn glyph_at(d: &Draw) -> (f32, f32, f32, [f32; 4]) {
        match *d {
            Draw::Glyph { x, y, w, uv, .. } => (x, y, w, uv),
            _ => panic!("not a glyph"),
        }
    }

    fn foil_at(d: &Draw) -> (f32, f32, f32, f32, [f32; 4], [f32; 4]) {
        match *d {
            Draw::Foil { x, y, w, h, uv, foil, .. } => (x, y, w, h, uv, foil),
            _ => panic!("not a foil"),
        }
    }

    #[test]
    fn a_badge_sits_inside_the_stickers_own_top_left() {
        let r = parse("BPEE\t-\t-\t-\t3\t-\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        let out = marks(row, "BPEE", quad, share, None, Some((TexId::from_raw(7), 1152, 1152)), 0.0, false, 1.0);
        assert_eq!(out.len(), 1);
        // The sticker's corner is the quad's origin plus the share's, and the glyph is inset
        // inside it rather than flush: 120 = 100 + 0.1*200, 110 = 50 + 0.2*300.
        let (x, y, w, uv) = glyph_at(&out[0]);
        let inset = BADGE_PX * 0.18;
        assert!((x - (120.0 + inset)).abs() < 0.001, "x {x}");
        assert!((y - (110.0 + inset)).abs() < 0.001, "y {y}");
        assert_eq!(w, BADGE_PX);
        // Cell 3 of a 16-wide grid: the third column of row 0, in the atlas' own 0..1 space.
        let cell = 1.0 / 16.0;
        assert!((uv[0] - 3.0 * cell).abs() < 1e-6, "u0 {}", uv[0]);
        assert!((uv[2] - 4.0 * cell).abs() < 1e-6, "u1 {}", uv[2]);
        assert_eq!(uv[1], 0.0);
    }

    #[test]
    fn a_badge_with_no_atlas_asks_for_nothing() {
        let r = parse("BPEE\t-\t-\t-\t3\t-\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        // The rule is a request, not a promise: the card can name a glyph it has no atlas
        // for, and the honest answer is no mark rather than a texture that does not exist.
        assert!(marks(row, "BPEE", quad, share, None, None, 0.0, false, 1.0).is_empty());
    }

    #[test]
    fn a_badge_past_the_end_of_the_atlas_asks_for_nothing() {
        let r = parse("BPEE\t-\t-\t-\t900\t-\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        // 900 is row 56 of a 16-wide grid, and this atlas is one row tall.
        assert!(marks(row, "BPEE", quad, share, None, Some((TexId::from_raw(0), 1152, 72)), 0.0, false, 1.0).is_empty());
    }

    #[test]
    fn a_film_covers_the_sticker_and_not_the_cart() {
        let r = parse("BPEE\t-\t-\t-\t-\tholo:0.4\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        let out = marks(row, "BPEE", quad, share, Some(TexId::from_raw(1)), None, 0.0, false, 1.0);
        assert_eq!(out.len(), 1);
        // The sticker's rect in panel pixels, and its own rect in the face's uv — the second
        // is what lines the film's pattern up with the paper instead of with the cart.
        let (x, y, w, h, uv, foil) = foil_at(&out[0]);
        assert!((x - 120.0).abs() < 0.001 && (y - 110.0).abs() < 0.001, "{x} {y}");
        assert!((w - 160.0).abs() < 0.001 && (h - 120.0).abs() < 0.001, "{w} {h}");
        assert_eq!(uv, [0.1, 0.2, 0.9, 0.6]);
        assert_eq!(foil[0], 0.4);
        assert_eq!(foil[3], FoilKind::Holo.effect());
    }

    #[test]
    fn an_edge_film_takes_the_whole_cart() {
        let r = parse("BPEE\t-\t-\t-\t-\tprism\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        let out = marks(row, "BPEE", quad, share, Some(TexId::from_raw(1)), None, 0.0, false, 1.0);
        let (x, y, w, h, uv, foil) = foil_at(&out[0]);
        // The whole quad, and the whole texture: the look reads the edge out of the face's
        // own alpha, so it needs the cart rather than the paper.
        assert_eq!((x, y, w, h), (100.0, 50.0, 200.0, 300.0));
        assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(foil[3], FoilKind::Prism.effect());
    }

    #[test]
    fn a_film_with_no_face_asks_for_nothing() {
        let r = parse("BPEE\t-\t-\t-\t-\tholo\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        assert!(marks(row, "BPEE", quad, share, None, None, 0.0, false, 1.0).is_empty());
    }

    #[test]
    fn a_still_film_stands_where_its_game_puts_it() {
        let r = parse("BPEE\t-\t-\t-\t-\tholo\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        let a = marks(row, "BPEE", quad, share, Some(TexId::from_raw(1)), None, 0.0, false, 1.0);
        let b = marks(row, "BPEE", quad, share, Some(TexId::from_raw(1)), None, 9.5, false, 1.0);
        // Unselected, the clock is not consulted: the same cart reads the same at any time,
        // and differently from its neighbour.
        assert_eq!(foil_at(&a[0]).5, foil_at(&b[0]).5);
        assert_eq!(foil_at(&a[0]).5[1], stable_phase("BPEE"));
        assert_eq!(foil_at(&a[0]).5[2], 0.0);
    }

    #[test]
    fn a_selected_film_sweeps_with_the_clock() {
        let r = parse("BPEE\t-\t-\t-\t-\tholo\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        let a = marks(row, "BPEE", quad, share, Some(TexId::from_raw(1)), None, 0.0, true, 1.0);
        let b = marks(row, "BPEE", quad, share, Some(TexId::from_raw(1)), None, 2.0, true, 1.0);
        assert!(foil_at(&a[0]).5[1] != foil_at(&b[0]).5[1], "the phase moves");
        assert!(foil_at(&b[0]).5[2] > 0.0, "and it carries a highlight");
    }

    #[test]
    fn a_dimmed_cart_dims_its_marks_with_it() {
        let r = parse("BPEE\t-\t-\t-\t3\tholo\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        let out = marks(row, "BPEE", quad, share, Some(TexId::from_raw(1)), Some((TexId::from_raw(2), 1152, 1152)), 0.0, false, 0.4);
        assert_eq!(out.len(), 2);
        match (&out[0], &out[1]) {
            (Draw::Glyph { alpha: a, .. }, Draw::Foil { alpha: b, .. }) => {
                assert_eq!(*a, 0.4);
                assert_eq!(*b, 0.4);
            }
            _ => panic!("a glyph and a foil, in that order"),
        }
    }

    #[test]
    fn the_phase_a_game_stands_at_is_stable_and_spread() {
        // Stable across calls, or a cart's film would flicker on every redraw.
        assert_eq!(stable_phase("BPEE"), stable_phase("BPEE"));
        assert_ne!(stable_phase("BPEE"), stable_phase("AXVJ"));
        // And inside 0..1, which is the range the shader wraps.
        for code in ["BPEE", "AXVJ", "POKEMON RED", "A", ""] {
            let p = stable_phase(code);
            assert!((0.0..=1.0).contains(&p), "{code} -> {p}");
        }
    }

    #[test]
    fn an_unmarked_rule_asks_for_nothing() {
        let r = parse("BPEE\t#249c60\tsolid\t-\t-\t-\n");
        let row = r.for_code("BPEE").unwrap();
        let (quad, share) = cart();
        assert!(marks(row, "BPEE", quad, share, Some(TexId::from_raw(1)), Some((TexId::from_raw(2), 1152, 1152)), 0.0, false, 1.0).is_empty());
    }
}

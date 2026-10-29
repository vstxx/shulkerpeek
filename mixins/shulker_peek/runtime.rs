// ShulkerPeek: a shulker box's contents as a 9 x 3 grid in its tooltip, under
// the lines Horizon already shows. This file is both the mod's Cargo library
// (its tests) and the `crate::game::shulker_peek` module the bundle adds to the
// game, so it uses plain Rust types only: the hook adapters in `tooltip.rs`,
// `list.rs`, `settings.rs` and `click.rs` turn the game's types into these.

use std::sync::atomic::{AtomicU8, Ordering};

/// The mod's id (its settings folder and its row in Options > Mods), name and
/// version.
pub const ID: &str = "shulker_peek";
pub const NAME: &str = "ShulkerPeek";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str =
    "A shulker box's contents as a 9 x 3 grid in its tooltip, as the box's own screen shows them.";

/// A shulker box's slots (`ShulkerBoxBlockEntity.CONTAINER_SIZE`), as its
/// screen lays them out.
pub const COLUMNS: usize = 9;
pub const ROWS: usize = 3;
pub const SLOTS: usize = COLUMNS * ROWS;
/// A slot's pitch in GUI pixels; the item sits 1 pixel inside it.
pub const SLOT: f32 = 18.0;
pub const GRID_W: f32 = COLUMNS as f32 * SLOT;
pub const GRID_H: f32 = ROWS as f32 * SLOT;
/// The grid's background: the slots of the box's own screen texture, so a
/// resource pack's look carries over. The texture is 256 x 256 texels at 1x;
/// the region is its 9 x 3 slots at (7, 17).
pub const TEXTURE: &str = "gui/container/shulker_box";
pub const TEXTURE_SIZE: f32 = 256.0;
pub const TEXTURE_REGION: [f32; 4] = [7.0, 17.0, GRID_W, GRID_H];
/// Entries read from a container component: vanilla's codec refuses more
/// (`ItemContainerContents.CODEC`, `sizeLimitedListOf(256)`); a malformed
/// list longer than that is cut here rather than walked.
pub const MAX_ENTRIES: usize = 256;

/// Whether an item id is a shulker box: the plain one and the 16 dyed ones
/// (`storage::is_shulker_box` in Horizon).
pub fn is_shulker_box(item: &str) -> bool {
    item.ends_with("shulker_box")
}

/// The box's 27 slots: for each, the index of the component entry it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    slots: [Option<u16>; SLOTS],
}

impl Grid {
    /// From a container component's entries, in their order: each entry's
    /// slot and whether it holds an item. As vanilla fills the box
    /// (`ItemContainerContents.fromSlots`, then `copyInto` its 27 slots), a
    /// later entry for a slot replaces an earlier one, an empty stack leaves
    /// the slot empty, and slots past the last are not in the box.
    pub fn new(entries: impl IntoIterator<Item = (u8, bool)>) -> Self {
        let mut slots = [None; SLOTS];
        for (i, (slot, present)) in entries.into_iter().take(MAX_ENTRIES).enumerate() {
            if let Some(cell) = slots.get_mut(usize::from(slot)) {
                // `i` < MAX_ENTRIES, which fits.
                *cell = present.then_some(i as u16);
            }
        }
        Self { slots }
    }
    /// The entry slot `slot` shows.
    pub fn get(&self, slot: usize) -> Option<usize> {
        self.slots.get(slot).copied().flatten().map(usize::from)
    }
    /// The slots that hold an item, in slot order, with their entries.
    pub fn occupied(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        (0..SLOTS).filter_map(|slot| Some((slot, self.get(slot)?)))
    }
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
}

/// Where the tooltip goes: its text's top-left corner and the box's size
/// (as Horizon's `tooltip_geometry` gives them), and the grid's top-left
/// corner, under the text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub grid: [f32; 2],
}

/// The tooltip of `lines` lines at most `text_w` wide, for the cursor at
/// `cursor` on a GUI of `screen` pixels, with the grid under the text.
/// Horizon's `tooltip_geometry` (`DefaultTooltipPositioner`), the box at
/// least as wide as the grid and as much taller as it, plus the 2 pixels
/// a bundle's contents get: right of the cursor, or left of it when it
/// would leave the screen; up from the bottom edge, never above the top.
pub fn layout(text_w: f32, lines: usize, cursor: (f32, f32), screen: (f32, f32)) -> Layout {
    let w = text_w.max(GRID_W);
    let text_h = if lines <= 1 {
        8.0
    } else {
        8.0 + 2.0 + (lines - 1) as f32 * 10.0
    };
    let h = text_h + 2.0 + GRID_H;
    let (width, height) = screen;
    let mut x = cursor.0 + 12.0;
    let mut y = cursor.1 - 12.0;
    if x + w > width - 4.0 {
        x = (x - 28.0 - w).max(4.0);
    }
    if y + h + 3.0 > height {
        y = height - h - 3.0;
    }
    y = y.max(4.0);
    Layout {
        x,
        y,
        w,
        h,
        grid: [x, y + text_h + 2.0],
    }
}

/// Where slot `slot`'s item is drawn, from the grid's corner.
pub fn item_offset(slot: usize) -> [f32; 2] {
    let (column, row) = (slot % COLUMNS, slot / COLUMNS);
    [column as f32 * SLOT + 1.0, row as f32 * SLOT + 1.0]
}

/// The atlas coordinates `[u0, v0, u1, v1]` of `TEXTURE_REGION`, from the
/// whole texture's: by fraction, so a resource pack's larger texture maps
/// the same slots.
pub fn region_uv([u0, v0, u1, v1]: [f32; 4]) -> [f32; 4] {
    let [x, y, w, h] = TEXTURE_REGION;
    let (du, dv) = ((u1 - u0) / TEXTURE_SIZE, (v1 - v0) / TEXTURE_SIZE);
    let (u, v) = (u0 + x * du, v0 + y * dv);
    [u, v, u + w * du, v + h * dv]
}

/// `DyeColor.getTextureDiffuseColor` of the 16 dyes, by name.
const DYES: [(&str, u32); 16] = [
    ("white", 0xF9FFFE),
    ("orange", 0xF9801D),
    ("magenta", 0xC74EBD),
    ("light_blue", 0x3AB3DA),
    ("yellow", 0xFED83D),
    ("lime", 0x80C71F),
    ("pink", 0xF38BAA),
    ("gray", 0x474F52),
    ("light_gray", 0x9D9D97),
    ("cyan", 0x169C9C),
    ("purple", 0x8932B8),
    ("blue", 0x3C44AA),
    ("brown", 0x835432),
    ("green", 0x5E7C16),
    ("red", 0xB02E26),
    ("black", 0x1D1D21),
];

/// The grid's tint for a dyed box (`minecraft:red_shulker_box`): its dye's
/// colour halfway to white, so the slots stay light enough to read; none
/// for the plain box and anything else.
pub fn tint(item: &str) -> Option<[u8; 3]> {
    let name = item
        .strip_prefix("minecraft:")?
        .strip_suffix("_shulker_box")?;
    let &(_, rgb) = DYES.iter().find(|d| d.0 == name)?;
    let lighten = |c: u32| (255 - (255 - (c & 0xFF)) / 2) as u8;
    Some([lighten(rgb >> 16), lighten(rgb >> 8), lighten(rgb)])
}

/// What the player chose on the mod's settings page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Show the grid only while Shift is held.
    pub shift: bool,
    /// Show the (empty) grid for an empty box too.
    pub empty: bool,
    /// Tint the grid with a dyed box's colour.
    pub dyed: bool,
}

impl Default for Settings {
    /// Always shown; an empty box keeps Horizon's tooltip; plain slots.
    fn default() -> Self {
        Self {
            shift: false,
            empty: false,
            dyed: false,
        }
    }
}

impl Settings {
    /// From the stored values (one file per key); the defaults for the ones
    /// missing or unknown.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let is = |key: &str, value: &str| get(key).is_some_and(|v| v.trim() == value);
        Self {
            shift: is("show", "shift"),
            empty: is("empty", "grid"),
            dyed: is("color", "dye"),
        }
    }
    pub fn stored(&self) -> [(&'static str, &'static str); 3] {
        [
            ("show", if self.shift { "shift" } else { "always" }),
            ("empty", if self.empty { "grid" } else { "hidden" }),
            ("color", if self.dyed { "dye" } else { "plain" }),
        ]
    }
    /// The settings page's buttons.
    pub fn labels(&self) -> [&'static str; 3] {
        [
            if self.shift {
                "Show contents: While holding Shift"
            } else {
                "Show contents: Always"
            },
            if self.empty {
                "Empty boxes: Show the empty grid"
            } else {
                "Empty boxes: No grid"
            },
            if self.dyed {
                "Grid colour: The box's dye"
            } else {
                "Grid colour: Plain"
            },
        ]
    }
    pub fn click(&mut self, button: usize) {
        match button {
            0 => self.shift = !self.shift,
            1 => self.empty = !self.empty,
            2 => self.dyed = !self.dyed,
            _ => {}
        }
    }
    fn bits(self) -> u8 {
        LOADED | u8::from(self.shift) << 1 | u8::from(self.empty) << 2 | u8::from(self.dyed) << 3
    }
    fn from_bits(bits: u8) -> Self {
        Self {
            shift: bits & 1 << 1 != 0,
            empty: bits & 1 << 2 != 0,
            dyed: bits & 1 << 3 != 0,
        }
    }
}

/// The settings in effect, with `LOADED` once read: a tooltip reads them
/// with one atomic load, never from a file.
static SETTINGS: AtomicU8 = AtomicU8::new(0);
const LOADED: u8 = 1;

/// Reads the settings through `get` and keeps them. The game lists its
/// native mods when it starts, which calls this.
pub fn load(get: impl Fn(&str) -> Option<String>) -> Settings {
    let settings = Settings::read(get);
    set(settings);
    settings
}

/// The settings kept; the defaults until they are read.
pub fn settings() -> Settings {
    Settings::from_bits(SETTINGS.load(Ordering::Relaxed))
}

/// The settings kept, read through `get` first if they never were.
pub fn settings_or_load(get: impl Fn(&str) -> Option<String>) -> Settings {
    let bits = SETTINGS.load(Ordering::Relaxed);
    if bits & LOADED == 0 {
        load(get)
    } else {
        Settings::from_bits(bits)
    }
}

/// Keeps new settings (the settings page changed them).
pub fn set(settings: Settings) {
    SETTINGS.store(settings.bits(), Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        |key| pairs.iter().find(|p| p.0 == key).map(|p| p.1.to_owned())
    }

    #[test]
    fn shulker_boxes_are_the_plain_and_the_sixteen_dyed() {
        assert!(is_shulker_box("minecraft:shulker_box"));
        for (dye, _) in DYES {
            assert!(is_shulker_box(&format!("minecraft:{dye}_shulker_box")));
        }
        for other in [
            "minecraft:chest",
            "minecraft:barrel",
            "minecraft:bundle",
            "",
        ] {
            assert!(!is_shulker_box(other), "{other}");
        }
    }

    #[test]
    fn an_empty_component_is_an_empty_grid() {
        let grid = Grid::new([]);
        assert!(grid.is_empty());
        assert_eq!(grid.occupied().count(), 0);
        assert_eq!(grid.get(0), None);
    }

    #[test]
    fn sparse_slots_keep_their_places_and_entries() {
        // Entries out of slot order, as a component may list them.
        let grid = Grid::new([(26, true), (0, true), (13, true)]);
        assert_eq!(
            grid.occupied().collect::<Vec<_>>(),
            [(0, 1), (13, 2), (26, 0)]
        );
        assert_eq!(grid.get(13), Some(2));
        assert_eq!(grid.get(1), None);
        assert!(!grid.is_empty());
    }

    #[test]
    fn a_full_box_fills_all_27_slots_in_order() {
        let grid = Grid::new((0..27u8).map(|slot| (slot, true)));
        let occupied: Vec<_> = grid.occupied().collect();
        assert_eq!(occupied.len(), SLOTS);
        assert!(occupied.iter().all(|&(slot, entry)| slot == entry));
    }

    #[test]
    fn invalid_slots_and_empty_stacks_show_nothing() {
        // Past the box (27..=255), an empty stack, then a later duplicate.
        let grid = Grid::new([(27, true), (255, true), (5, false), (3, true), (3, true)]);
        assert_eq!(grid.occupied().collect::<Vec<_>>(), [(3, 4)]);
        assert_eq!(grid.get(27), None);
        assert_eq!(grid.get(usize::MAX), None);
        // A later empty entry empties the slot, as `fromSlots` sets it.
        let grid = Grid::new([(4, true), (4, false)]);
        assert!(grid.is_empty());
    }

    #[test]
    fn a_malformed_list_is_read_only_up_to_vanilla_s_limit() {
        let grid = Grid::new((0..10_000u32).map(|i| ((i % 27) as u8, true)));
        // The last entry read is index 255, for slot 255 % 27 = 12.
        assert_eq!(grid.get(12), Some(MAX_ENTRIES - 1));
        assert_eq!(grid.occupied().count(), SLOTS);
    }

    #[test]
    fn items_sit_one_pixel_inside_their_slots() {
        assert_eq!(item_offset(0), [1.0, 1.0]);
        assert_eq!(item_offset(8), [145.0, 1.0]);
        assert_eq!(item_offset(9), [1.0, 19.0]);
        assert_eq!(item_offset(26), [145.0, 37.0]);
    }

    #[test]
    fn layout_matches_horizons_tooltip_beside_the_cursor() {
        // A one-line name narrower than the grid: the box takes the grid's width.
        let l = layout(40.0, 1, (100.0, 50.0), (427.0, 240.0));
        assert_eq!((l.x, l.y, l.w), (112.0, 38.0, GRID_W));
        assert_eq!(l.h, 8.0 + 2.0 + GRID_H);
        assert_eq!(l.grid, [112.0, 48.0]);
        // Three lines: the first is spaced (8 + 2), then 10 each.
        let l = layout(200.0, 3, (100.0, 50.0), (427.0, 240.0));
        assert_eq!(l.w, 200.0);
        assert_eq!(l.grid[1] - l.y, 8.0 + 2.0 + 20.0 + 2.0);
    }

    #[test]
    fn layout_stays_on_screen_at_every_gui_scale() {
        // A 1920 x 1080 window at GUI scales 1 to 4, and a small one.
        let screens = [
            (1920.0, 1080.0),
            (960.0, 540.0),
            (640.0, 360.0),
            (480.0, 270.0),
            (320.0, 240.0),
        ];
        for screen in screens {
            for cursor in [
                (0.0, 0.0),
                (screen.0, screen.1),
                (screen.0 - 1.0, 5.0),
                (3.0, screen.1 - 2.0),
            ] {
                for lines in [1, 4, 8] {
                    let l = layout(120.0, lines, cursor, screen);
                    assert!(
                        l.x >= 4.0 && l.x + l.w <= screen.0 - 4.0,
                        "{screen:?} {cursor:?} {l:?}"
                    );
                    assert!(
                        l.y >= 4.0 && l.y + l.h + 3.0 <= screen.1,
                        "{screen:?} {cursor:?} {l:?}"
                    );
                    assert!(l.grid[1] + GRID_H <= l.y + l.h);
                }
            }
        }
    }

    /// The mod's own work per frame while a full box's tooltip shows:
    /// `cargo test --release -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn measure_normalize_and_layout() {
        let entries: Vec<(u8, bool)> = (0..27u8).rev().map(|slot| (slot, true)).collect();
        let rounds = 1_000_000u32;
        let started = std::time::Instant::now();
        let mut sum = 0usize;
        for i in 0..rounds {
            let grid = Grid::new(std::hint::black_box(&entries).iter().copied());
            let cursor = (f32::from((i % 400) as u16), 50.0);
            let l = layout(std::hint::black_box(120.0), 6, cursor, (427.0, 240.0));
            sum += grid.occupied().map(|(s, e)| s ^ e).sum::<usize>() + l.x as usize;
        }
        let ns = started.elapsed().as_nanos() as f64 / f64::from(rounds);
        println!("normalize 27 slots + layout + slot walk: {ns:.1} ns per frame ({sum})");
    }

    #[test]
    fn layout_flips_left_at_the_right_edge_and_rises_from_the_bottom() {
        let l = layout(10.0, 1, (400.0, 230.0), (427.0, 240.0));
        assert_eq!(l.x, 400.0 + 12.0 - 28.0 - GRID_W);
        assert_eq!(l.y + l.h + 3.0, 240.0);
    }

    #[test]
    fn the_region_is_the_screens_slots_whatever_the_atlas_rect() {
        let [u0, v0, u1, v1] = region_uv([0.0, 0.0, 1.0, 1.0]);
        assert_eq!(
            [u0 * 256.0, v0 * 256.0, u1 * 256.0, v1 * 256.0],
            [7.0, 17.0, 169.0, 71.0]
        );
        // The same texture packed at (0.5, 0.25), half the atlas wide.
        let [u0, v0, u1, _] = region_uv([0.5, 0.25, 1.0, 0.75]);
        assert_eq!(u0, 0.5 + 7.0 / 512.0);
        assert_eq!(v0, 0.25 + 17.0 / 512.0);
        assert_eq!(u1, 0.5 + 169.0 / 512.0);
    }

    #[test]
    fn dyed_boxes_tint_their_grid_and_the_plain_box_does_not() {
        assert_eq!(tint("minecraft:shulker_box"), None);
        assert_eq!(tint("minecraft:chest"), None);
        assert_eq!(tint("othermod:red_shulker_box"), None);
        assert_eq!(tint("minecraft:white_shulker_box"), Some([252, 255, 255]));
        assert_eq!(tint("minecraft:black_shulker_box"), Some([142, 142, 144]));
        assert_eq!(
            tint("minecraft:light_blue_shulker_box"),
            Some([157, 217, 237])
        );
    }

    #[test]
    fn settings_read_click_and_store() {
        let s = Settings::read(stored(&[]));
        assert_eq!(s, Settings::default());
        assert_eq!(
            s.labels(),
            [
                "Show contents: Always",
                "Empty boxes: No grid",
                "Grid colour: Plain"
            ]
        );
        let mut s = Settings::read(stored(&[
            ("show", "shift\n"),
            ("empty", "grid"),
            ("color", "nonsense"),
        ]));
        assert_eq!(
            s,
            Settings {
                shift: true,
                empty: true,
                dyed: false
            }
        );
        s.click(0);
        s.click(2);
        s.click(9);
        assert_eq!(
            s.stored(),
            [("show", "always"), ("empty", "grid"), ("color", "dye")]
        );
        assert_eq!(Settings::read(stored(&s.stored())), s);
    }

    #[test]
    fn kept_settings_survive_the_bit_round_trip() {
        for bits in 0..8u8 {
            let s = Settings {
                shift: bits & 1 != 0,
                empty: bits & 2 != 0,
                dyed: bits & 4 != 0,
            };
            assert_eq!(Settings::from_bits(s.bits()), s);
            assert_ne!(s.bits() & LOADED, 0);
        }
    }
}

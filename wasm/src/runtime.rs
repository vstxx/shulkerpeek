// ShulkerPeek (WASM port): the pure logic, with no dependency on the mod API so
// that it is tested in plain Rust. Ported from vstxx's original
// `mixins/shulker_peek/runtime.rs` (MIT): the grid normalisation, the item
// offsets, the dye tints and the settings are the original's; the tooltip's
// own placement (`layout`) is gone because the game now positions a tooltip
// component itself, and the texture region maps by size instead of by atlas
// fraction (see `src_rect`).

use std::sync::atomic::{AtomicU8, Ordering};

/// The mod's id (its settings folder and its row in Options > Mods).
pub const ID: &str = "shulkerpeek_wasm";

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

/// The plain shulker box and the 16 dyed ones, in `DyeColor` order.
pub const SHULKER_BOXES: [&str; 17] = [
    "minecraft:shulker_box",
    "minecraft:white_shulker_box",
    "minecraft:orange_shulker_box",
    "minecraft:magenta_shulker_box",
    "minecraft:light_blue_shulker_box",
    "minecraft:yellow_shulker_box",
    "minecraft:lime_shulker_box",
    "minecraft:pink_shulker_box",
    "minecraft:gray_shulker_box",
    "minecraft:light_gray_shulker_box",
    "minecraft:cyan_shulker_box",
    "minecraft:purple_shulker_box",
    "minecraft:blue_shulker_box",
    "minecraft:brown_shulker_box",
    "minecraft:green_shulker_box",
    "minecraft:red_shulker_box",
    "minecraft:black_shulker_box",
];

/// Whether an item id is a shulker box: the plain one and the 16 dyed ones.
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

/// The grid component's size in GUI pixels, as `tooltip_component` answers it.
/// The game reserves it under the text and places it (screen edges included);
/// the 2 pixels under the name are the game's own spacing.
pub fn component_size() -> [f32; 2] {
    [GRID_W, GRID_H]
}

/// Where slot `slot`'s item is drawn, from the grid's corner.
pub fn item_offset(slot: usize) -> [f32; 2] {
    let (column, row) = (slot % COLUMNS, slot / COLUMNS);
    [column as f32 * SLOT + 1.0, row as f32 * SLOT + 1.0]
}

/// The part of the box's screen texture holding the 9 x 3 slots, in the
/// sprite's own pixels as `DrawContext::sprite` takes them, for a sprite of
/// `size` pixels (`AtlasRegion::size`). By fraction of the 256 x 256 default
/// texture, so a resource pack's larger texture maps the same slots. None for
/// a sprite without area.
pub fn src_rect(size: [u32; 2]) -> Option<[f32; 4]> {
    if size[0] == 0 || size[1] == 0 {
        return None;
    }
    let (sx, sy) = (size[0] as f32 / TEXTURE_SIZE, size[1] as f32 / TEXTURE_SIZE);
    let [x, y, w, h] = TEXTURE_REGION;
    Some([x * sx, y * sy, w * sx, h * sy])
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

/// `tint` as the `0xAARRGGBB` a sprite is multiplied by (opaque).
pub fn tint_argb(item: &str) -> Option<u32> {
    let [r, g, b] = tint(item)?;
    Some(0xFF00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b))
}

/// The settings' keys, as registered with the game.
pub const KEY_ENABLED: &str = "enabled";
pub const KEY_SHOW: &str = "show";
pub const KEY_EMPTY: &str = "empty";
pub const KEY_COLOR: &str = "color";

/// What the player chose on the mod's settings page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The mod's on/off switch.
    pub enabled: bool,
    /// Show the grid only while Shift is held.
    pub shift: bool,
    /// Show the (empty) grid for an empty box too.
    pub empty: bool,
    /// Tint the grid with a dyed box's colour.
    pub dyed: bool,
}

impl Default for Settings {
    /// On; always shown; an empty box keeps the plain tooltip; plain slots.
    fn default() -> Self {
        Self {
            enabled: true,
            shift: false,
            empty: false,
            dyed: false,
        }
    }
}

impl Settings {
    /// From the typed values the game keeps (`setting_bool`, `setting_choice`):
    /// choice index 1 is the second option of each (Shift, empty grid, dye);
    /// the defaults for a value the game doesn't give.
    pub fn from_values(
        enabled: Option<bool>,
        show: Option<u32>,
        empty: Option<u32>,
        color: Option<u32>,
    ) -> Self {
        Self {
            enabled: enabled.unwrap_or(true),
            shift: show == Some(1),
            empty: empty == Some(1),
            dyed: color == Some(1),
        }
    }
    fn bits(self) -> u8 {
        LOADED
            | u8::from(self.enabled) << 1
            | u8::from(self.shift) << 2
            | u8::from(self.empty) << 3
            | u8::from(self.dyed) << 4
    }
    fn from_bits(bits: u8) -> Self {
        Self {
            enabled: bits & 1 << 1 != 0,
            shift: bits & 1 << 2 != 0,
            empty: bits & 1 << 3 != 0,
            dyed: bits & 1 << 4 != 0,
        }
    }
}

/// The settings in effect, with `LOADED` once read: a tooltip reads them
/// with one atomic load, not four host calls.
static SETTINGS: AtomicU8 = AtomicU8::new(0);
const LOADED: u8 = 1;

/// The settings kept, if they were read since the last `invalidate`.
pub fn kept() -> Option<Settings> {
    let bits = SETTINGS.load(Ordering::Relaxed);
    (bits & LOADED != 0).then(|| Settings::from_bits(bits))
}

/// Keeps `settings` as the ones in effect.
pub fn keep(settings: Settings) {
    SETTINGS.store(settings.bits(), Ordering::Relaxed);
}

/// Forgets the kept settings: the next tooltip reads them again.
pub fn invalidate() {
    SETTINGS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_registered_ids_are_exactly_the_seventeen_boxes() {
        assert_eq!(SHULKER_BOXES.len(), 17);
        assert!(SHULKER_BOXES.iter().all(|id| is_shulker_box(id)));
        let mut unique = SHULKER_BOXES.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 17);
        assert_eq!(SHULKER_BOXES[0], "minecraft:shulker_box");
        for (dye, _) in DYES {
            let id = format!("minecraft:{dye}_shulker_box");
            assert!(SHULKER_BOXES.contains(&id.as_str()), "{id}");
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
    fn an_entry_past_the_limit_is_never_read() {
        // Entry 256 would fill slot 0; the 256 first leave it empty.
        let entries = (0..256).map(|_| (200u8, true)).chain([(0u8, true)]);
        assert!(Grid::new(entries).is_empty());
    }

    #[test]
    fn items_sit_one_pixel_inside_their_slots() {
        assert_eq!(item_offset(0), [1.0, 1.0]);
        assert_eq!(item_offset(8), [145.0, 1.0]);
        assert_eq!(item_offset(9), [1.0, 19.0]);
        assert_eq!(item_offset(26), [145.0, 37.0]);
    }

    #[test]
    fn the_component_is_the_grid_and_nothing_else() {
        let [w, h] = component_size();
        assert_eq!((w, h), (162.0, 54.0));
        // A tooltip component must answer finite, positive sizes.
        assert!(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0);
        // The last item stays inside the component.
        let [x, y] = item_offset(SLOTS - 1);
        assert!(x + 16.0 <= w && y + 16.0 <= h);
    }

    #[test]
    fn the_region_is_the_screens_slots_whatever_the_texture_size() {
        // The default 256 x 256 texture: the 9 x 3 slots at (7, 17).
        assert_eq!(src_rect([256, 256]), Some([7.0, 17.0, 162.0, 54.0]));
        // A pack's 512 x 512 texture maps the same slots, by fraction.
        assert_eq!(src_rect([512, 512]), Some([14.0, 34.0, 324.0, 108.0]));
        // A non-square one scales each axis on its own.
        assert_eq!(src_rect([512, 256]), Some([14.0, 17.0, 324.0, 54.0]));
        assert_eq!(src_rect([0, 256]), None);
        assert_eq!(src_rect([256, 0]), None);
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
    fn the_tint_is_opaque_argb_and_only_dyed_boxes_have_one() {
        assert_eq!(tint_argb("minecraft:shulker_box"), None);
        assert_eq!(tint_argb("minecraft:white_shulker_box"), Some(0xFFFC_FFFF));
        assert_eq!(tint_argb("minecraft:black_shulker_box"), Some(0xFF8E_8E90));
        let dyed = SHULKER_BOXES.iter().filter(|id| tint_argb(id).is_some());
        assert_eq!(dyed.count(), 16);
    }

    #[test]
    fn settings_default_to_on_always_no_empty_grid_plain() {
        let s = Settings::from_values(None, None, None, None);
        assert_eq!(s, Settings::default());
        assert!(s.enabled && !s.shift && !s.empty && !s.dyed);
    }

    #[test]
    fn settings_read_the_games_typed_values() {
        let s = Settings::from_values(Some(false), Some(1), Some(1), Some(1));
        assert_eq!(
            s,
            Settings {
                enabled: false,
                shift: true,
                empty: true,
                dyed: true
            }
        );
        // The first option of each choice, and an out-of-range index, are the defaults.
        let s = Settings::from_values(Some(true), Some(0), Some(0), Some(7));
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn kept_settings_survive_the_bit_round_trip() {
        for bits in 0..16u8 {
            let s = Settings {
                enabled: bits & 1 != 0,
                shift: bits & 2 != 0,
                empty: bits & 4 != 0,
                dyed: bits & 8 != 0,
            };
            assert_eq!(Settings::from_bits(s.bits()), s);
            assert_ne!(s.bits() & LOADED, 0);
        }
    }

    #[test]
    fn settings_are_kept_until_invalidated() {
        // The only test touching the static: it never races another.
        invalidate();
        assert_eq!(kept(), None);
        let s = Settings::from_values(Some(true), Some(1), Some(0), Some(1));
        keep(s);
        assert_eq!(kept(), Some(s));
        // Even all-false settings count as read.
        let off = Settings::from_values(Some(false), Some(0), Some(0), Some(0));
        keep(off);
        assert_eq!(kept(), Some(off));
        invalidate();
        assert_eq!(kept(), None);
    }

    /// The mod's own work per frame while a full box's tooltip shows:
    /// `cargo test --release -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn measure_normalize_and_slot_walk() {
        let entries: Vec<(u8, bool)> = (0..27u8).rev().map(|slot| (slot, true)).collect();
        let rounds = 1_000_000u32;
        let started = std::time::Instant::now();
        let mut sum = 0usize;
        for _ in 0..rounds {
            let grid = Grid::new(std::hint::black_box(&entries).iter().copied());
            sum += grid.occupied().map(|(s, e)| s ^ e).sum::<usize>();
        }
        let ns = started.elapsed().as_nanos() as f64 / f64::from(rounds);
        println!("normalize 27 slots + slot walk: {ns:.1} ns per frame ({sum})");
    }
}

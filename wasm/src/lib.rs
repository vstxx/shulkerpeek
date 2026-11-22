// ShulkerPeek (WASM port of vstxx's native mod, MIT): a shulker box's contents
// as a 9 x 3 grid in its tooltip, under the lines the game already shows.
//
// The hooks here only translate the game's types into `runtime.rs`' plain
// ones; the rules (which slots show, where items sit, which tint, which
// settings) live there and are tested without the game.
//
// Reads only: no stack is changed, nothing is sent to a server.

mod runtime;

use std::sync::OnceLock;

use mod_api::{
    ClientHost, Hooks, Identifier, ItemPart, ItemStack, Registrar, RegistrationContext,
    SettingChangedEvent, SettingSpec, Tag, TooltipComponentDrawEvent, TooltipComponentEvent,
    TooltipDisplay,
};
use runtime::{Grid, Settings};

/// The component holding a box's items (`ItemContainerContents`): a list of
/// `{slot, item}` entries.
const CONTAINER: &str = "minecraft:container";
/// What a bundle holds; a box that also has some is left to the game.
const BUNDLE: &str = "minecraft:bundle_contents";
/// The slots' texture in the GUI atlas (vanilla sprites are named by path).
const TEXTURE_ID: &str = "minecraft:gui/container/shulker_box";
/// A slot's colour when a resource pack has no box screen (vanilla's grey).
const SLOT_FALLBACK: u32 = 0xFF8B_8B8B;

const LABEL_ENABLED: &str = "shulkerpeek_wasm.setting.enabled";
const LABEL_SHOW: &str = "shulkerpeek_wasm.setting.show";
const LABEL_EMPTY: &str = "shulkerpeek_wasm.setting.empty";
const LABEL_COLOR: &str = "shulkerpeek_wasm.setting.color";

/// The four settings, default value first as in the original.
fn setting_specs() -> [SettingSpec; 4] {
    [
        SettingSpec::toggle(runtime::KEY_ENABLED, LABEL_ENABLED, true)
            .tooltip("shulkerpeek_wasm.setting.enabled.tooltip"),
        SettingSpec::choice(
            runtime::KEY_SHOW,
            LABEL_SHOW,
            [
                "shulkerpeek_wasm.setting.show.always",
                "shulkerpeek_wasm.setting.show.shift",
            ],
            0,
        ),
        SettingSpec::choice(
            runtime::KEY_EMPTY,
            LABEL_EMPTY,
            [
                "shulkerpeek_wasm.setting.empty.hidden",
                "shulkerpeek_wasm.setting.empty.grid",
            ],
            0,
        ),
        SettingSpec::choice(
            runtime::KEY_COLOR,
            LABEL_COLOR,
            [
                "shulkerpeek_wasm.setting.color.plain",
                "shulkerpeek_wasm.setting.color.dye",
            ],
            0,
        ),
    ]
}

/// The settings in effect: one atomic load while they are kept, four typed
/// reads (cached by the game) when they are not. Nothing is kept until the
/// game answers, so a read before a world is open doesn't freeze the defaults.
fn settings(host: &dyn ClientHost) -> Settings {
    if let Some(kept) = runtime::kept() {
        return kept;
    }
    let enabled = host.setting_bool(runtime::KEY_ENABLED);
    let read = Settings::from_values(
        enabled,
        host.setting_choice(runtime::KEY_SHOW),
        host.setting_choice(runtime::KEY_EMPTY),
        host.setting_choice(runtime::KEY_COLOR),
    );
    if enabled.is_some() {
        runtime::keep(read);
    }
    read
}

/// The stack's own value of component `id`, borrowed from its patch.
fn component<'a>(stack: &'a ItemStack, id: &str) -> Option<&'a Tag> {
    stack
        .components()?
        .as_nbt()
        .iter()
        .find(|(key, _)| key.0.iter().copied().eq(id.encode_utf16()))
        .map(|(_, tag)| tag)
}

/// The container component's entries, none when the box has no contents.
fn entries(stack: &ItemStack) -> &[Tag] {
    match component(stack, CONTAINER) {
        Some(Tag::List(list)) => list,
        _ => &[],
    }
}

/// An entry's slot; anything that isn't a byte-sized int is past the box.
fn entry_slot(entry: &Tag) -> u8 {
    match entry.get("slot") {
        Some(Tag::Int(slot)) => u8::try_from(*slot).unwrap_or(u8::MAX),
        _ => u8::MAX,
    }
}

/// Whether an entry's `item` holds an item: an id that isn't air, and a count
/// above zero (a missing count is one, as the game reads it).
fn holds_item(entry: &Tag) -> bool {
    let Some(item) = entry.get("item") else {
        return false;
    };
    let Some(Tag::String(id)) = item.get("id") else {
        return false;
    };
    if id.0.iter().copied().eq("minecraft:air".encode_utf16()) {
        return false;
    }
    match item.get("count") {
        None => true,
        Some(Tag::Byte(n)) => *n > 0,
        Some(Tag::Short(n)) => *n > 0,
        Some(Tag::Int(n)) => *n > 0,
        Some(Tag::Long(n)) => *n > 0,
        Some(_) => false,
    }
}

/// Everything both hooks need, or none when the tooltip stays the game's: the
/// mod is off, the stack hides its tooltip or its container, it also holds a
/// bundle's contents, Shift is required and not held, or the box is empty and
/// the empty grid isn't asked for.
fn plan<'a>(host: &dyn ClientHost, stack: &'a ItemStack) -> Option<(Settings, &'a [Tag], Grid)> {
    let settings = settings(host);
    if !settings.enabled {
        return None;
    }
    if matches!(component(stack, BUNDLE), Some(Tag::List(held)) if !held.is_empty())
        || !TooltipDisplay::for_stack(stack).shows(CONTAINER)
    {
        return None;
    }
    if settings.shift && !host.modifiers().shift() {
        return None;
    }
    let list = entries(stack);
    let grid = Grid::new(list.iter().map(|entry| (entry_slot(entry), holds_item(entry))));
    if grid.is_empty() && !settings.empty {
        return None;
    }
    Some((settings, list, grid))
}

/// The box's slots as a sprite lookup: parsed once, never per frame.
fn texture_id() -> Option<&'static Identifier> {
    static ID: OnceLock<Option<Identifier>> = OnceLock::new();
    ID.get_or_init(|| Identifier::parse(TEXTURE_ID).ok()).as_ref()
}

/// What each of the 17 boxes carries: the tint its dye gives the grid.
#[derive(Clone)]
struct Peek {
    tint: Option<u32>,
}

#[mod_api::horizon_part]
impl ItemPart for Peek {
    fn tooltip_component(
        &self,
        host: &dyn ClientHost,
        event: &mut TooltipComponentEvent<'_>,
    ) -> Option<[f32; 2]> {
        plan(host, event.stack).map(|_| runtime::component_size())
    }

    fn tooltip_component_draw(&self, host: &dyn ClientHost, event: &mut TooltipComponentDrawEvent<'_>) {
        let stack = event.stack;
        let Some((settings, list, grid)) = plan(host, stack) else {
            return;
        };
        let [x, y] = event.origin;
        let tint = self.tint.filter(|_| settings.dyed);
        // The box's own slots, by fraction of the texture so a larger pack's
        // maps the same; plain squares when no pack has the texture.
        let drew = texture_id()
            .and_then(|id| host.gui_sprite(id))
            .and_then(|region| runtime::src_rect(region.size))
            .is_some_and(|src| {
                event.draw.sprite(
                    runtime::TEXTURE,
                    src,
                    [x, y, runtime::GRID_W, runtime::GRID_H],
                    tint,
                )
            });
        if !drew {
            for slot in 0..runtime::SLOTS {
                let [dx, dy] = runtime::item_offset(slot);
                event.draw.fill([x + dx, y + dy, 16.0, 16.0], SLOT_FALLBACK);
            }
        }
        for (slot, entry) in grid.occupied() {
            let Some(Tag::Compound(item)) = list[entry].get("item") else {
                continue;
            };
            let Some(item) = ItemStack::from_nbt(host, item) else {
                continue;
            };
            let [dx, dy] = runtime::item_offset(slot);
            // The model, its durability bar and its count, as a slot draws them.
            event.draw.gui_draw_item_with(&item, [x + dx, y + dy], None);
        }
    }
}

#[derive(Default)]
struct ShulkerPeek;

#[mod_api::horizon_mod]
impl Hooks for ShulkerPeek {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for id in runtime::SHULKER_BOXES {
            let key = r.item_key(id);
            r.item_part(
                key,
                Peek {
                    tint: runtime::tint_argb(id),
                },
            );
        }
        for spec in setting_specs() {
            r.register_setting(spec)
                .expect("ShulkerPeek's settings are valid and registered once");
        }
    }

    /// The player changed a setting: read all of them again at the next tooltip.
    fn setting_changed(&self, _: &mut dyn ClientHost, _: &mut SettingChangedEvent<'_>) {
        runtime::invalidate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN_US: &str = include_str!("../assets/shulkerpeek_wasm/lang/en_us.json");
    const FR_FR: &str = include_str!("../assets/shulkerpeek_wasm/lang/fr_fr.json");

    #[test]
    fn every_setting_passes_the_games_own_check() {
        for spec in setting_specs() {
            assert_eq!(spec.check(), Ok(()), "{}", spec.key);
        }
    }

    #[test]
    fn settings_registered_are_the_ones_the_runtime_reads() {
        let keys: Vec<String> = setting_specs().into_iter().map(|s| s.key).collect();
        assert_eq!(
            keys,
            [
                runtime::KEY_ENABLED,
                runtime::KEY_SHOW,
                runtime::KEY_EMPTY,
                runtime::KEY_COLOR
            ]
        );
    }

    #[test]
    fn the_defaults_are_the_runtimes_defaults() {
        let [enabled, show, empty, color] = setting_specs();
        assert_eq!(
            Settings::from_values(
                Some(enabled.default_value() == mod_api::SettingValue::Toggle(true)),
                match show.default_value() {
                    mod_api::SettingValue::Choice(i) => Some(i as u32),
                    _ => None,
                },
                match empty.default_value() {
                    mod_api::SettingValue::Choice(i) => Some(i as u32),
                    _ => None,
                },
                match color.default_value() {
                    mod_api::SettingValue::Choice(i) => Some(i as u32),
                    _ => None,
                },
            ),
            Settings::default()
        );
    }

    #[test]
    fn every_translation_key_exists_in_each_language_file() {
        for spec in setting_specs() {
            let mut keys = vec![spec.label.clone()];
            keys.extend(spec.tooltip.clone());
            if let mod_api::SettingKind::Choice { options, .. } = &spec.kind {
                keys.extend(options.clone());
            }
            for key in keys {
                for (name, file) in [("en_us", EN_US), ("fr_fr", FR_FR)] {
                    assert!(file.contains(&format!("\"{key}\"")), "{key} missing in {name}");
                }
            }
        }
    }

    #[test]
    fn a_slot_that_is_not_a_byte_is_past_the_box() {
        assert_eq!(entry_slot(&Tag::Int(3)), u8::MAX); // no `slot` key at all
        let entry = |slot: i32| {
            Tag::compound(vec![("slot", Tag::Int(slot))])
        };
        assert_eq!(entry_slot(&entry(5)), 5);
        assert_eq!(entry_slot(&entry(-1)), u8::MAX);
        assert_eq!(entry_slot(&entry(300)), u8::MAX);
    }

    #[test]
    fn an_entry_holds_an_item_only_with_a_real_id_and_a_positive_count() {
        let entry = |id: &str, count: Option<i32>| {
            let mut item = vec![("id", Tag::String(id.into()))];
            item.extend(count.map(|n| ("count", Tag::Int(n))));
            Tag::compound(vec![("slot", Tag::Int(0)), ("item", Tag::compound(item))])
        };
        assert!(holds_item(&entry("minecraft:diamond", Some(64))));
        assert!(holds_item(&entry("minecraft:stone", None)));
        assert!(!holds_item(&entry("minecraft:diamond", Some(0))));
        assert!(!holds_item(&entry("minecraft:air", Some(1))));
        assert!(!holds_item(&Tag::compound(vec![("slot", Tag::Int(0))])));
    }
}

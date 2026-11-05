// ShulkerPeek against Horizon's own code, inside the game's test binary: the
// slots and visibility against stacks decoded by Horizon from saved item data.
// (Horizon lays the tooltip out itself through the `tooltip_component` hook
// point, so there is no layout of the mod's own to check.)
#[cfg(test)]
mod tests {
    use crate::game::shulker_peek as m;
    use horizon_client::{item_tooltip::TooltipDisplay, items::ItemStack};
    use horizon_world::nbt::Tag;

    fn entry(slot: i32, id: &str, count: i32) -> Tag {
        let item = Tag::compound([("id", Tag::String(id.into())), ("count", Tag::Int(count))]);
        Tag::compound([("slot", Tag::Int(slot)), ("item", item)])
    }

    /// A red shulker box as Horizon reads it from saved data.
    fn shulker(entries: Vec<Tag>, more: Vec<(&str, Tag)>) -> ItemStack {
        let mut components = vec![("minecraft:container", Tag::List(entries))];
        components.extend(more);
        let tag = Tag::compound([
            ("id", Tag::String("minecraft:red_shulker_box".into())),
            ("count", Tag::Int(1)),
            ("components", Tag::compound(components)),
        ]);
        horizon_client::save::item_from_nbt(&tag).unwrap()
    }

    /// The slots as the tooltip hook reads them.
    fn slots_of(stack: &ItemStack) -> (m::Grid, &[(u8, ItemStack)]) {
        let contents = stack.data().map_or(&[][..], |d| &d.container[..]);
        let grid = m::Grid::new(
            contents
                .iter()
                .map(|(slot, s)| (*slot, s.count > 0 && &*s.item != "minecraft:air")),
        );
        (grid, contents)
    }

    #[test]
    fn decoded_slots_keep_their_places_counts_and_order() {
        let stack = shulker(
            vec![
                entry(26, "minecraft:diamond", 64),
                entry(0, "minecraft:apple", 5),
                entry(40, "minecraft:stone", 1),
                entry(13, "minecraft:stick", 1),
                entry(0, "minecraft:golden_apple", 2),
            ],
            vec![],
        );
        let (grid, contents) = slots_of(&stack);
        let shown: Vec<_> = grid
            .occupied()
            .map(|(slot, e)| (slot, &*contents[e].1.item, contents[e].1.count))
            .collect();
        assert_eq!(
            shown,
            [
                (0, "minecraft:golden_apple", 2),
                (13, "minecraft:stick", 1),
                (26, "minecraft:diamond", 64),
            ]
        );
    }

    #[test]
    fn a_full_box_and_an_empty_box() {
        let full = shulker(
            (0..27)
                .map(|i| entry(i, "minecraft:cobblestone", i + 1))
                .collect(),
            vec![],
        );
        let (grid, contents) = slots_of(&full);
        let counts: Vec<u32> = grid.occupied().map(|(_, e)| contents[e].1.count).collect();
        assert_eq!(counts, (1..=27).collect::<Vec<u32>>());
        let empty = horizon_client::save::item_from_nbt(&Tag::compound([
            ("id", Tag::String("minecraft:shulker_box".into())),
            ("count", Tag::Int(1)),
        ]))
        .unwrap();
        assert!(slots_of(&empty).0.is_empty());
        assert!(TooltipDisplay::for_stack(&empty).shows("minecraft:container"));
    }

    #[test]
    fn a_hidden_container_or_tooltip_is_left_to_horizon() {
        let hidden_list = Tag::List(vec![Tag::String("minecraft:container".into())]);
        let hidden = shulker(
            vec![entry(0, "minecraft:diamond", 1)],
            vec![(
                "minecraft:tooltip_display",
                Tag::compound([("hidden_components", hidden_list)]),
            )],
        );
        assert!(!TooltipDisplay::for_stack(&hidden).shows("minecraft:container"));
        let no_tooltip = shulker(
            vec![entry(0, "minecraft:diamond", 1)],
            vec![(
                "minecraft:tooltip_display",
                Tag::compound([("hide_tooltip", Tag::Byte(1))]),
            )],
        );
        assert!(!TooltipDisplay::for_stack(&no_tooltip).shows("minecraft:container"));
        let shown = shulker(vec![entry(0, "minecraft:diamond", 1)], vec![]);
        assert!(TooltipDisplay::for_stack(&shown).shows("minecraft:container"));
    }

    #[test]
    fn custom_names_stay_on_the_stacks_the_grid_draws() {
        let named = Tag::compound([
            ("id", Tag::String("minecraft:diamond_sword".into())),
            ("count", Tag::Int(1)),
            (
                "components",
                Tag::compound([("minecraft:custom_name", Tag::String("Edge".into()))]),
            ),
        ]);
        let stack = shulker(
            vec![Tag::compound([("slot", Tag::Int(4)), ("item", named)])],
            vec![],
        );
        let (grid, contents) = slots_of(&stack);
        let (slot, e) = grid.occupied().next().unwrap();
        assert_eq!(slot, 4);
        assert_eq!(contents[e].1.custom_name().as_deref(), Some("Edge"));
    }
}

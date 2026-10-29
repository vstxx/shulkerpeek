// What the server sends a client of a shulker box: its contents
// (`ItemContainerContents`), which Horizon's `network_stack` leaves out, so the
// client's tooltip has them as a vanilla server's client does. The
// `horizon_client::lan::hub::shulker_peek_sync` module; the game sets
// `ENABLED` from Options > Mods. It changes only what a client is told about
// an item, never the item.

use crate::items::ItemStack;
use crate::lan::hub::Hub;
use core::ops::ControlFlow;
use horizon_protocol::buf::{Codec, Writer};
use horizon_protocol::item::{Component, Stack, Template, component};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the mod is on (`Game::native_on`); one game process, so the
/// window's choice reaches its server thread.
pub static ENABLED: AtomicBool = AtomicBool::new(true);

thread_local! {
    /// Set while `send` encodes a box through `Hub::stack` itself.
    static ENCODING: Cell<bool> = const { Cell::new(false) };
}

/// `Hub::stack` for a shulker box with contents its tooltip shows: the
/// stack Horizon encodes, plus the contents. `Continue` for everything else.
pub fn send(hub: &mut Hub, s: &Option<ItemStack>) -> ControlFlow<Option<Stack>> {
    let Some(stack) = s else {
        return ControlFlow::Continue(());
    };
    let Some(data) = stack.data() else {
        return ControlFlow::Continue(());
    };
    if !stack.item.ends_with("shulker_box")
        || data.container.is_empty()
        || ENCODING.get()
        || !ENABLED.load(Ordering::Relaxed)
        // The server sends no `tooltip_display`: a box that hides its
        // contents keeps them from the client instead.
        || !crate::item_tooltip::TooltipDisplay::for_stack(stack).shows("minecraft:container")
    {
        return ControlFlow::Continue(());
    }
    ENCODING.set(true);
    let base = hub.stack(s);
    let entries: Vec<(u8, Option<Stack>)> = data
        .container
        .iter()
        .map(|(slot, item)| (*slot, hub.stack(&Some(item.clone()))))
        .collect();
    ENCODING.set(false);
    let Some(mut base) = base else {
        return ControlFlow::Break(None);
    };
    if let Some(bytes) = container_bytes(&entries) {
        base.added
            .push((component::CONTAINER, Component::Raw(bytes)));
    }
    ControlFlow::Break(Some(base))
}

/// `ItemContainerContents.STREAM_CODEC`: one optional template per slot up
/// to the last that holds one (a later entry for a slot wins, as
/// `fromSlots` sets it). None when no slot holds anything.
pub fn container_bytes(entries: &[(u8, Option<Stack>)]) -> Option<Vec<u8>> {
    let mut slots: Vec<Option<&Stack>> = Vec::new();
    for (slot, stack) in entries {
        let slot = usize::from(*slot);
        if slots.len() <= slot {
            slots.resize(slot + 1, None);
        }
        slots[slot] = stack.as_ref();
    }
    while slots.last().is_some_and(Option::is_none) {
        slots.pop();
    }
    if slots.is_empty() {
        return None;
    }
    let mut w = Writer::new();
    w.varint(slots.len() as i32);
    for slot in slots {
        w.bool(slot.is_some());
        if let Some(stack) = slot {
            Template::write(stack, &mut w);
        }
    }
    Some(w.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_protocol::buf::Reader;
    use horizon_protocol::item::Slot;
    use horizon_protocol::registry::{Builtin, Dynamic, Ids};

    const ITEMS: [&str; 5] = [
        "minecraft:air",
        "minecraft:red_shulker_box",
        "minecraft:diamond",
        "minecraft:apple",
        "minecraft:diamond_sword",
    ];

    fn item_id(name: &str) -> i32 {
        ITEMS.iter().position(|i| *i == name).unwrap_or(0) as i32
    }

    /// A box encoded as `send` encodes it, through the packet's bytes, then
    /// decoded as a client decodes it (`item_sync::from_network`).
    fn round_trip(entries: &[(u8, ItemStack)]) -> ItemStack {
        let mut components = vec![String::new(); 100];
        components[component::CONTAINER as usize] = "minecraft:container".into();
        components[component::CUSTOM_NAME as usize] = "minecraft:custom_name".into();
        let builtin = Builtin {
            registries: [
                (
                    "minecraft:item".to_owned(),
                    Ids::from_names(ITEMS.iter().map(|s| s.to_string()).collect()),
                ),
                (
                    "minecraft:data_component_type".to_owned(),
                    Ids::from_names(components),
                ),
            ]
            .into(),
        };
        let dynamic = Dynamic {
            registries: Default::default(),
        };
        let wire = |s: &ItemStack| {
            crate::lan::guest::stack_to_network(item_id, |_, _| None, &Some(s.clone()))
        };
        let mut boxed = wire(&ItemStack::new("minecraft:red_shulker_box", 1)).unwrap();
        let encoded: Vec<_> = entries.iter().map(|(slot, s)| (*slot, wire(s))).collect();
        if let Some(bytes) = container_bytes(&encoded) {
            boxed
                .added
                .push((component::CONTAINER, Component::Raw(bytes)));
        }
        let mut w = Writer::new();
        Slot::write(&Some(boxed), &mut w);
        let mut r = Reader::new(&w.data);
        let read = Slot::read(&mut r).unwrap().unwrap();
        assert_eq!(r.remaining(), 0);
        crate::item_sync::from_network(&read, &builtin, &dynamic)
    }

    fn shown(stack: &ItemStack) -> Vec<(u8, String, u32, Option<String>)> {
        let mut slots: Vec<_> = stack
            .data()
            .map(|d| d.container.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|(slot, s)| {
                (
                    slot,
                    s.item.to_string(),
                    s.count,
                    s.custom_name().map(str::to_owned),
                )
            })
            .collect();
        slots.sort();
        slots
    }

    #[test]
    fn a_boxs_contents_reach_the_client_in_their_slots() {
        let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
        sword.data_mut().custom_name = Some("Edge".into());
        let back = round_trip(&[
            (26, ItemStack::new("minecraft:diamond", 64)),
            (0, ItemStack::new("minecraft:apple", 5)),
            (4, sword),
        ]);
        assert_eq!(
            shown(&back),
            [
                (0, "minecraft:apple".into(), 5, None),
                (4, "minecraft:diamond_sword".into(), 1, Some("Edge".into())),
                (26, "minecraft:diamond".into(), 64, None),
            ]
        );
    }

    #[test]
    fn a_full_box_and_a_later_duplicate() {
        let full: Vec<_> = (0..27u8)
            .map(|i| (i, ItemStack::new("minecraft:diamond", u32::from(i) + 1)))
            .collect();
        let back = round_trip(&full);
        let counts: Vec<u32> = shown(&back).iter().map(|s| s.2).collect();
        assert_eq!(counts, (1..=27).collect::<Vec<u32>>());
        let back = round_trip(&[
            (3, ItemStack::new("minecraft:apple", 1)),
            (3, ItemStack::new("minecraft:diamond", 2)),
        ]);
        assert_eq!(shown(&back), [(3, "minecraft:diamond".into(), 2, None)]);
    }

    #[test]
    fn nothing_to_send_sends_no_component() {
        assert_eq!(container_bytes(&[]), None);
        assert_eq!(container_bytes(&[(5, None)]), None);
        assert!(shown(&round_trip(&[])).is_empty());
    }
}

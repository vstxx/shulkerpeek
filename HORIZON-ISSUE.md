# Draft bug report for horizon-rust/horizon

Horizon's AGENTS.md asks that an issue written by an agent say so on its first line. Fill in the name and keep
the line when you file it.

---

> 🤖 Filed by vstxx's AI agent (Claude Code, Claude Opus 5.5), not by vstxx themselves.

**Title:** Shulker box contents are not sent to clients: no "N × item" tooltip lines in single player or on LAN

**What happens**

- I hover a filled shulker box in the default single-player mode (the server thread). The tooltip shows only the
  name. Vanilla shows up to five "64 × Diamond" lines (`ItemContainerContents.addToTooltip`).
- With `--in-frame` the lines are there.
- `/data get entity @s Inventory` shows the `minecraft:container` component on the server.

I reproduced this on Nightly `5e391eb` and on Nightly `a491181` (2026-10-29).

**Steps**

1. Create a new creative world (default mode).
2. Run `/item replace entity @s hotbar.0 with minecraft:red_shulker_box[minecraft:container=[{slot:0,item:{id:"minecraft:diamond",count:64}}]]`.
3. Open the inventory and hover the box.

**Cause**

`lan::guest::network_stack` doesn't write `minecraft:container` (`component::CONTAINER`, 77), although the
client decodes it (`item_sync.rs`, `container_tag`). In single player the window is a client of its server
thread, and LAN guests are clients too, so none of them get the contents.

The same function also leaves out `minecraft:tooltip_display`. Clients therefore can't honour a hidden
tooltip or hidden components on items the server sends.

**Suggested fix** (in `network_stack`, beside the bundle contents; `ItemContainerContents.STREAM_CODEC` is one
optional `ItemStackTemplate` per slot):

```rust
// ItemContainerContents: a shulker box's items, for its tooltip.
if !d.container.is_empty() {
    let mut slots: Vec<Option<Stack>> = Vec::new();
    for (slot, c) in &d.container {
        let slot = usize::from(*slot);
        if slots.len() <= slot {
            slots.resize(slot + 1, None);
        }
        slots[slot] = network_stack(item_id, synced_id, &Some(c.clone()));
    }
    while slots.last().is_some_and(Option::is_none) {
        slots.pop();
    }
    if !slots.is_empty() {
        let mut w = horizon_protocol::buf::Writer::new();
        w.varint(slots.len() as i32);
        for s in &slots {
            w.bool(s.is_some());
            if let Some(s) = s {
                <horizon_protocol::item::Template as horizon_protocol::buf::Codec<_>>::write(s, &mut w);
            }
        }
        stack.added.push((
            horizon_protocol::item::component::CONTAINER,
            horizon_protocol::item::Component::Raw(w.data),
        ));
    }
}
```

The same encoding, applied in the server's `Hub::stack` by the native mod ShulkerPeek, passes a round trip
through the client's `item_sync::from_network`:

- sparse slots, a full box, a later duplicate slot and a nested custom name all survive;
- in a running game, the tooltip lines and the contents appear in the default mode.

This exact patch hasn't been compiled inside Horizon.

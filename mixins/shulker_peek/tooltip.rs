// Before Horizon draws an item's tooltip (every screen's: the inventory, the
// containers, creative, the item panel, chat, trading). A shulker box whose
// tooltip shows its contents gets Horizon's own lines, box and text, with the
// box's slots under them; any other stack, a box the settings leave alone or
// one with a bundle's contents, goes on to Horizon's tooltip unchanged. Reads
// only: no stack is cloned or changed.
fn shulker_peek_tooltip(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    stack: &horizon_client::items::ItemStack,
    x: f32,
    y: f32,
) -> core::ops::ControlFlow<()> {
    use crate::game::shulker_peek as m;
    use core::ops::ControlFlow::{Break, Continue};
    use winit::keyboard::KeyCode;
    if !m::is_shulker_box(&stack.item) {
        return Continue(());
    }
    // Options > Mods' switch, also for what the server thread sends (sync.rs).
    let on = game.native_on(m::ID);
    horizon_client::lan::hub::shulker_peek_sync::ENABLED
        .store(on, std::sync::atomic::Ordering::Relaxed);
    if !on {
        return Continue(());
    }
    let data = stack.data();
    // `TooltipDisplay`: `hide_tooltip`, or `minecraft:container` hidden.
    if data.is_some_and(|d| !d.bundle_contents.is_empty())
        || !horizon_client::item_tooltip::TooltipDisplay::for_stack(stack)
            .shows("minecraft:container")
    {
        return Continue(());
    }
    let settings = m::settings();
    let held = |k| game.keys.contains(&k);
    if settings.shift && !held(KeyCode::ShiftLeft) && !held(KeyCode::ShiftRight) {
        return Continue(());
    }
    let contents = data.map_or(&[][..], |d| &d.container[..]);
    let grid = m::Grid::new(
        contents
            .iter()
            .map(|(slot, s)| (*slot, s.count > 0 && &*s.item != "minecraft:air")),
    );
    if grid.is_empty() && !settings.empty {
        return Continue(());
    }
    let game = &*game;
    let lines = game.stack_tooltip(stack);
    if lines.is_empty() {
        return Continue(());
    }
    let lines = crate::game::Game::resolved(&lines);
    let atlas = &game.gui.atlas;
    let text_w = lines
        .iter()
        .map(|(t, _)| atlas.font.width_text(t))
        .fold(0.0, f32::max);
    let at = m::layout(text_w, lines.len(), (x, y), (batch.width, batch.height));
    crate::game::Game::tooltip_box(batch, atlas, at.x, at.y, at.w, at.h);
    crate::game::Game::tooltip_text(batch, atlas, &lines, at.x, at.y);
    let [gx, gy] = at.grid;
    let tint = m::tint(&stack.item).filter(|_| settings.dyed);
    let color = tint.map_or(u32::MAX, |[r, g, b]| {
        horizon_client::hud::rgba(r, g, b, 255)
    });
    if let Some(mut r) = atlas.sprites.get(m::TEXTURE).copied() {
        [r.u0, r.v0, r.u1, r.v1] = m::region_uv([r.u0, r.v0, r.u1, r.v1]);
        batch.rect_uv(gx, gy, m::GRID_W, m::GRID_H, r, color);
    } else {
        // A resource pack without the box's screen: plain slot squares.
        let slot = horizon_client::hud::rgba(139, 139, 139, 255);
        for i in 0..m::SLOTS {
            let [ox, oy] = m::item_offset(i);
            batch.fill(atlas, gx + ox, gy + oy, 16.0, 16.0, slot);
        }
    }
    for (i, entry) in grid.occupied() {
        let [ox, oy] = m::item_offset(i);
        game.draw_item(batch, &contents[entry].1, gx + ox, gy + oy);
    }
    Break(())
}

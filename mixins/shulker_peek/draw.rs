// The tooltip hook point `tooltip_component_draw`: the box's slots inside the
// tooltip Horizon drew, at `origin`, under its lines. The background is the
// slot area of the box's own screen texture (tinted with a dyed box's colour
// when the settings ask), then each item as a slot draws it, with its count.
fn shulker_peek_draw(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
    size: [f32; 2],
) {
    use crate::game::shulker_peek as m;
    if size != [m::GRID_W, m::GRID_H] {
        return;
    }
    let Some((grid, contents)) = crate::game::shulker_peek_game::shown(game, stack) else {
        return;
    };
    let [gx, gy] = origin;
    let tint = m::tint(&stack.item).filter(|_| m::settings().dyed);
    let drawn = match tint {
        // `native_sprite` has no colour, so a tinted grid uses the atlas.
        Some([r, g, b]) => match game.gui.atlas.sprites.get(m::TEXTURE).copied() {
            Some(mut region) => {
                [region.u0, region.v0, region.u1, region.v1] =
                    m::region_uv([region.u0, region.v0, region.u1, region.v1]);
                let color = horizon_client::hud::rgba(r, g, b, 255);
                batch.rect_uv(gx, gy, m::GRID_W, m::GRID_H, region, color);
                true
            }
            None => false,
        },
        None => game.native_sprite(
            batch,
            m::TEXTURE,
            m::TEXTURE_REGION,
            [gx, gy, m::GRID_W, m::GRID_H],
        ),
    };
    if !drawn {
        // A resource pack without the box's screen: plain slot squares.
        for i in 0..m::SLOTS {
            let [ox, oy] = m::item_offset(i);
            game.native_fill(batch, [gx + ox, gy + oy, 16.0, 16.0], m::SLOT_FALLBACK_ARGB);
        }
    }
    for (i, entry) in grid.occupied() {
        let [ox, oy] = m::item_offset(i);
        let (x, y) = (gx + ox, gy + oy);
        let item = &contents[entry].1;
        if game.native_item_stack(batch, item, [x, y], 16.0) && item.count != 1 {
            // `GuiGraphicsExtractor.itemCount`: right-aligned, white, shadowed.
            let count = item.count.to_string();
            let w = game.native_text_width(&count);
            game.native_text(batch, &count, [x + 17.0 - w, y + 9.0], 0xFF_FFFF, true);
        }
    }
}

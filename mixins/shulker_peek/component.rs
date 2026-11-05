// The tooltip hook point `tooltip_component`: a shulker box whose contents
// show reserves the grid's room under the tooltip's lines. Another mod's
// component, asked first, is left alone.
fn shulker_peek_component(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    size: &mut Option<[f32; 2]>,
) {
    use crate::game::shulker_peek as m;
    if size.is_none() && crate::game::shulker_peek_game::shown(game, stack).is_some() {
        *size = Some([m::GRID_W, m::GRID_H]);
    }
}

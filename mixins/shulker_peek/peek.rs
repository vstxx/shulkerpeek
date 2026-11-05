// What a stack's tooltip shows, for the two tooltip hook points: the size
// (`component.rs`) and the drawing (`draw.rs`) ask the same question, so they
// always agree. The `crate::game::shulker_peek_game` module.

use super::shulker_peek as m;
use horizon_client::items::ItemStack;

/// The grid for `stack`'s tooltip, with the entries its slots show; none when
/// the tooltip stays Horizon's own: anything but a shulker box, the mod off,
/// a stack that hides `minecraft:container` (`TooltipDisplay`), one that also
/// has a bundle's contents, Shift not held while the settings ask for it, or
/// an empty box while empty grids are off.
pub(super) fn shown<'s>(
    game: &super::Game<'_>,
    stack: &'s ItemStack,
) -> Option<(m::Grid, &'s [(u8, ItemStack)])> {
    if !m::is_shulker_box(&stack.item) || !game.native_on(m::ID) {
        return None;
    }
    let data = stack.data();
    if data.is_some_and(|d| !d.bundle_contents.is_empty())
        || !horizon_client::item_tooltip::TooltipDisplay::for_stack(stack)
            .shows("minecraft:container")
    {
        return None;
    }
    let settings = m::settings();
    if settings.shift && !m::shift_held() {
        return None;
    }
    let contents = data.map_or(&[][..], |d| &d.container[..]);
    let grid = m::Grid::new(
        contents
            .iter()
            .map(|(slot, s)| (*slot, s.count > 0 && &*s.item != "minecraft:air")),
    );
    if grid.is_empty() && !settings.empty {
        return None;
    }
    Some((grid, contents))
}

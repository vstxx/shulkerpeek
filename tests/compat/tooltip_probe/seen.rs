// Counts the tooltips drawn, before ShulkerPeek decides.
fn tooltip_probe_seen(stack: &horizon_client::items::ItemStack) {
    crate::game::tooltip_probe::seen(&stack.item);
}

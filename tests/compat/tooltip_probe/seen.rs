// Counts the tooltips asked for a component, before ShulkerPeek answers.
fn tooltip_probe_seen(stack: &horizon_client::items::ItemStack) {
    crate::game::tooltip_probe::seen(&stack.item);
}

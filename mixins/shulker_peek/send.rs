// Before the server encodes a stack for its clients (single player's window,
// LAN guests): a shulker box's contents go with it, as a vanilla server
// sends them (sync.rs). Every other stack goes on to Horizon's encoding.
fn shulker_peek_send(
    hub: &mut crate::lan::hub::Hub,
    s: &Option<crate::items::ItemStack>,
) -> core::ops::ControlFlow<Option<horizon_protocol::item::Stack>> {
    crate::lan::hub::shulker_peek_sync::send(hub, s)
}

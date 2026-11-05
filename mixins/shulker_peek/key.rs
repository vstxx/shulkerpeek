// The input hook point `key`: whether Shift is held, for the "while holding
// Shift" setting.
fn shulker_peek_key(key: &str, pressed: bool) {
    crate::game::shulker_peek::shift_key(key, pressed);
}

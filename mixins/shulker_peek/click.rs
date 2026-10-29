// A click on the mod's settings page: change the setting and keep it.
fn shulker_peek_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::shulker_peek as m;
    if mod_id != m::ID {
        return;
    }
    let mut settings = m::settings_or_load(|key| game.native_setting(m::ID, key));
    settings.click(button);
    m::set(settings);
    for (key, value) in settings.stored() {
        game.native_set_setting(m::ID, key, value);
    }
}

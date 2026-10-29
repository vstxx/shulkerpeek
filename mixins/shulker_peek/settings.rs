// The mod's settings page: one button per setting.
fn shulker_peek_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::shulker_peek as m;
    if mod_id != m::ID {
        return;
    }
    let game = &*game;
    let settings = m::settings_or_load(|key| game.native_setting(m::ID, key));
    buttons.extend(settings.labels().map(String::from));
}

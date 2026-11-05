// The mod's row in Options > Mods. The game lists its native mods when it
// starts: the settings are read here, so a tooltip never reads a file.
fn shulker_peek_listed(
    game: &mut crate::game::Game<'_>,
    mods: &mut Vec<crate::game::native_hooks::NativeMod>,
) {
    use crate::game::shulker_peek as m;
    let game = &*game;
    m::load(|key| game.native_setting(m::ID, key));
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}

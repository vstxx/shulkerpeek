# Validation of 0.2.0 (2026-11-22, Windows 11)

Built with `cargo-horizon-mod` and the SDK of `f037744a79f9982c47876f49c5a3067279f88706` (the registry's pin).
rustfmt, `clippy -D warnings`, 22 unit tests and `cargo horizon-mod check` pass.

In game on Nightly 2026-11-22 (`2b77287de099c9316278ba7ac4d83590fdb7ef30`), driven with `horizon-ctl` offscreen at
1280 x 720 in single player, checked on screenshots:

| Case | Result |
|---|---|
| Red, plain, black, cyan and pink boxes | Grid under the name, items in their slots |
| Sparse box; full box of 27 stacks | Correct slots and counts |
| Damaged sword inside | Durability bar |
| Lore | Stays, below the grid |
| Empty box | No grid; a grid with "Show the empty grid" |
| `hidden_components: ["minecraft:container"]` | No grid |
| `hide_tooltip` | No tooltip |
| An item that is no box | Untouched |
| While holding Shift | No grid until Shift is held |
| The box's dye | Tinted slots |
| Off | No grid |
| Restart | All four settings kept |
| Survival inventory, creative inventory, chest | Grid shows |
| GUI scale auto and 1; near the bottom edge | Stays on screen |

Not tested: running on `f037744` itself, Linux and macOS, LAN guests and remote servers, resource packs, the other
dye colours in game, and other mods installed alongside.

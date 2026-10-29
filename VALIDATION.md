# Validation

Run on 2026-10-29 on Windows 11 (Intel i7-14650HX, 24 threads), with MSVC build tools and Rust 1.99.1 (the
launcher's native-mod toolchain). Horizon was `5e391eb67392941734df7ddd648728c4ed5302f6` (Nightly
2026-10-29), exported with `git archive` from a clone that was never checked out or modified. horizon-mixins
was `0d6b103c70633d8fd9443d8d0acd3aadaacedb06`, built with its own pinned Rust 1.95.0. The game's cargo
commands used the launcher's features (`--features horizon-client/mods`).

| Check | Result |
|---|---|
| `cargo fmt --all --check`; `rustfmt --edition 2024 --check mixins/shulker_peek/*.rs tests/*/*/*.rs` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | 14 passed, 1 ignored (the measurement) |
| `python tools/test_stage.py` | 3546 files staged. A repeated stage keeps all bytes and mtimes. The Horizon clone's status and HEAD are unchanged. The AST audit finds only the 5 planned HEAD statements and the 2 support modules. |
| `python tools/horizon.py check` | The real `horizon-client` binary type-checks, 0 warnings |
| `python tools/horizon.py test --with tests/native` | 22 passed inside the game: the 14 runtime tests and 5 parity tests in its binary, and 3 encoding tests in its library (below) |
| `python tools/horizon.py build` | `horizon-client` and `horizon-ctl` in the `fast` profile, 0 warnings (115.6 s of Cargo for the final rebuild). |
| `python tools/test_combined.py` | Builds with ShulkerPeek, all 6 official native mods (coords_hud, fps_overlay, keystrokes, minimap, recipes, zoom) and the tooltip probe: 34 injections and 10 modules. 0 warnings. The probe's HEAD runs before ShulkerPeek's cancellable one, in both the plan and the staged source. |
| `python tools/horizon.py package` | `shulker_peek-0.1.0.hnmod`, 13670 bytes, SHA-256 `e4e33cb698d91042b2ed2f93a1c935f279d16d83988115d53c0f33530cabeab5`. The bytes are identical when packaged again, and identical to the package CI builds on Linux. |
| `cargo horizon-mod check` (built from the pinned Horizon) | Accepted: `native-mod, format native-1/5e391eb…/0d6b103…, for 26.3` |

The parity tests in `tests/native` run in the game and compare ShulkerPeek with Horizon's own code:

- The layout equals the private `tooltip_geometry` with the grid as its extra height. Checked for 1, 2 and 6
  lines, 3 GUI sizes and 4 cursor positions.
- Stacks decoded by Horizon from saved item data give the right grid:
  - sparse, out-of-order, out-of-range and duplicate slots, with counts kept;
  - a full box with 27 different counts;
  - a box with no container component;
  - a nested custom name.
- `tooltip_display` hides the grid in both forms: a hidden `minecraft:container`, and `hide_tooltip`.

The encoding tests in `sync.rs` encode a box as the server hook does and send it through the packet's bytes.
The client's real decoder (`item_sync::from_network`) then gives back the same slots, counts and nested custom
name, for sparse slots, a full box and a later duplicate slot. An empty box sends no component.

Alternatives were tested with the real planner on the pinned export. AFTER_CALL on `draw_stack_tooltip`, RETURN
on `draw_stack_tooltip` and MODIFY_CONSTANT in `tooltip_geometry` all fail with `E_UNSUPPORTED_CONTROL_FLOW`,
because of the closures in those bodies. Cancellable HEAD plans successfully.

Measurement: `cargo test --release -- --ignored --nocapture` takes about 18.5 ns per frame for normalizing a full
box, the layout and the slot walk. Drawing isn't included.

## In-game run

The built game was driven with `horizon-ctl` on 2026-10-29, offscreen, creative, at a 1280 × 720 window.
It used the player's prepared 26.3 resources (from launcher version `nightly-2026-10-29-a491181`), read-only,
with fresh temporary `HORIZON_HOME`, launcher and XDG folders.

- **Options → Mods:** lists "ShulkerPeek 0.1.0: ON". Its settings page shows the three buttons. Clicking
  "Grid colour" saved `color` = `dye` in `horizon-mods/.horizon-mod-data/shulker_peek/`.
- **In-frame mode (`--in-frame`), hovering a red box in the creative inventory:**
  - The tooltip keeps Horizon's lines: the name, "64 × Diamond", "1 × Edge", "5 × Apple" and "1 × Stone".
  - Under them is the 9 × 3 grid, with the diamonds (64) in slot 0, the sword in slot 4, the apples (5) in
    slot 13 and the stone in slot 26.
  - The box moved up to stay on screen.
  - With the dye setting on, the grid is tinted red.
- **Default server-thread mode (what players run), before the fix:** the server had the box's contents
  (`data get entity @s Inventory`), but the client's copy had none. The tooltip showed only the name, with no
  grid and none of Horizon's own "item × count" lines either. The installed Nightly `a491181`, unmodded,
  behaves the same. The cause is that Horizon's server never writes `minecraft:container` in
  `lan::guest::network_stack` (see HORIZON-ISSUE.md).
- **Default server-thread mode, after the fix (`send.rs`/`sync.rs`):** hovering the same box shows Horizon's
  "64 × Diamond", "5 × Apple" and "1 × Stone" lines, and under them the grid with every item in its slot.

## Not done

- **The hook adapter has no automated test.** Horizon has no way to build a `Game` in tests. Its inputs
  (decoding and visibility) and its outputs (layout and slot placement) are covered by the parity tests, and it
  was exercised in the run above.
- **Not exercised in game:**
  - container screens other than creative;
  - the Shift setting, empty boxes, other GUI scales and resource packs;
  - a second player joining a LAN world (the hook covers them through the same `Hub::stack`);
  - joining a vanilla server.
- **Only Windows was run locally.** CI runs the same checks on Linux.

## Windows notes

- horizon-mixins `0d6b103` replaces an existing stage only on Linux (`E_ATOMIC_STAGE`). `tools/horizon.py`
  therefore applies into a fresh folder and copies only the changed files into the stage, as the launcher does.
- In the project's `target/` a fresh apply failed with `E_IO` (a file held by another process, most likely the
  editor or a scanner watching the project). The tool now applies into the system's temporary folder, retries
  up to 3 times, and copies only changed files into the stage.
- A Windows clone with `core.autocrlf=true` would export CRLF. The tool exports with `core.autocrlf=false`, so the
  source bytes match GitHub's archive and the hashes in `compatibility.json`.

#!/usr/bin/env python3
"""ShulkerPeek together with other native mods, as a player's set: Horizon's official native mods (a clone of
horizon-rust/horizon-mods: native/*) and the test-only tooltip probe (tests/compat), which hooks the same tooltip
method. Checks that the combined plan runs the probe's hook before ShulkerPeek's cancellable one, that the staged
source has them in that order, and that the real horizon-client binary type-checks with every mod applied.

    python tools/test_combined.py --horizon ../horizon --official ../horizon-mods/native [--target-dir DIR]
"""
import argparse
import json
import re
from pathlib import Path

import horizon

TARGET = "horizon_client::game::stations::Game<'_>::draw_stack_tooltip"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--horizon", type=Path, default=horizon.ROOT.parent / "horizon")
    parser.add_argument("--mixins", type=Path)
    parser.add_argument("--official", type=Path, default=horizon.ROOT.parent / "horizon-mods" / "native")
    parser.add_argument("--target-dir", type=Path, default=horizon.ROOT / "target" / "horizon-cargo")
    args = parser.parse_args()
    official = sorted(p.parent for p in args.official.resolve().glob("*/mixins/*/horizon-mixin.toml"))
    assert official, f"no native mods in {args.official}"
    extra = [*official, horizon.ROOT / "tests" / "compat"]
    with horizon.lock(horizon.ROOT / "target" / "stage.lock"):
        staged = horizon.stage(args.horizon.resolve(), horizon.mixins_root(args.mixins), extra)
    plan = json.loads(Path(staged["plan"]).read_text())
    order = [(i["mod_id"], i["injection_id"]) for i in plan["injections"] if i["target"] == TARGET]
    assert order == [("tooltip_probe", "seen"), ("shulker_peek", "tooltip")], order
    stations = (staged["stage"] / "crates/horizon-client/src/game/stations.rs").read_text()
    body = stations[stations.index("pub(super) fn draw_stack_tooltip("):]
    calls = re.findall(r"^\s+(tooltip_probe_seen|shulker_peek_tooltip)\(", body, re.M)[:2]
    assert calls == ["tooltip_probe_seen", "shulker_peek_tooltip"], calls
    report, result = horizon.cargo("check", staged["stage"], args.target_dir.resolve(),
                                   ["--features", "horizon-client/mods"])
    warnings = len(re.findall(r"^warning", result.stderr, re.M))
    summary = {"mods": staged["mods"].split("+"), "injections": staged["injections"],
               "modules": staged["modules"], "tooltip_hook_order": order, "warnings": warnings, **report}
    print(json.dumps(summary, indent=2))
    if result.returncode or warnings:
        print(result.stderr[-12000:])
        raise SystemExit(1)


if __name__ == "__main__":
    main()

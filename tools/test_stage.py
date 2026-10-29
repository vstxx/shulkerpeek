#!/usr/bin/env python3
"""Stage the bundle on the real pinned Horizon export twice and audit it: the Horizon clone is unchanged, the second
run changes no byte or mtime, the plan is the expected one, and only the planned HEAD statements and the support
module differ from the pristine Rust ASTs.

    python tools/test_stage.py --horizon ../horizon [--mixins ../horizon-mixins]
"""
import argparse
import json
import subprocess
from pathlib import Path

import horizon


def snapshot(root):
    return {p.relative_to(root).as_posix(): (horizon.digest(p.read_bytes()), p.stat().st_mtime_ns)
            for p in root.rglob("*") if p.is_file()}


def audit(original, stage, plan):
    subprocess.run(["cargo", "run", "--quiet", "--locked", "-p", "native-mod-source-audit", "--",
                    str(original), str(stage), str(plan)], cwd=horizon.ROOT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--horizon", type=Path, default=horizon.ROOT.parent / "horizon")
    parser.add_argument("--mixins", type=Path)
    args = parser.parse_args()
    clone = args.horizon.resolve()
    status = subprocess.check_output(["git", "-C", str(clone), "status", "--porcelain"])
    head = subprocess.check_output(["git", "-C", str(clone), "rev-parse", "HEAD"])
    mixins = horizon.mixins_root(args.mixins)
    with horizon.lock(horizon.ROOT / "target" / "stage.lock"):
        first = horizon.stage(clone, mixins)
        before = snapshot(first["stage"])
        second = horizon.stage(clone, mixins)
        assert second["stage"] == first["stage"] and not second["stage_changed"], second
        assert snapshot(second["stage"]) == before, "a repeated stage changed bytes or mtimes"
        plan = json.loads(first["plan"].read_text())
        assert plan["schema"] == horizon.COMPAT["mixin_schema"], plan["schema"]
        kinds = sorted((i["injection_id"], i["target"].rsplit("::", 1)[1], i["cancellable"], i["priority"])
                       for i in plan["injections"])
        assert kinds == [
            ("click", "native_settings_click", False, 100),
            ("list", "native_mods", False, 100),
            ("send", "stack", True, -1000),
            ("settings", "native_settings", False, 100),
            ("tooltip", "draw_stack_tooltip", True, -1000),
        ], kinds
        assert sorted(m["name"] for m in plan["modules"]) == [horizon.MOD_ID, f"{horizon.MOD_ID}_sync"]
        audit(first["original"], first["stage"], first["plan"])
    assert subprocess.check_output(["git", "-C", str(clone), "status", "--porcelain"]) == status
    assert subprocess.check_output(["git", "-C", str(clone), "rev-parse", "HEAD"]) == head
    print(json.dumps({"status": "ok", "horizon_clone_unchanged": True, "repeat_preserves_bytes_and_mtimes": True,
                      "only_planned_asts_changed": True, "files_staged": len(before)}, indent=2))


if __name__ == "__main__":
    main()

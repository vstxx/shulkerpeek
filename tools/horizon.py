#!/usr/bin/env python3
"""Export the pinned Horizon source, apply ShulkerPeek with horizon-mixins, and check, test, build or package it.

    python tools/horizon.py stage   --horizon ../horizon    # export + plan + apply; prints the stage
    python tools/horizon.py check   --horizon ../horizon    # cargo check the real horizon-client binary
    python tools/horizon.py test    --horizon ../horizon    # the runtime's tests inside the game's test binary
    python tools/horizon.py build   --horizon ../horizon    # fast profile: horizon-client and horizon-ctl
    python tools/horizon.py package                         # target/shulker_peek-<version>.hnmod
    --with DIR      apply another bundle too (repeatable): a combined build, as a player's set is
    --target-dir    a reusable Cargo target directory (default target/horizon-cargo)

Horizon's source is `git archive` of the pinned commit from a clone (`--horizon`); the clone is never checked out
or edited. horizon-mixins must be a clean checkout at the pinned commit (`--mixins`, `HORIZON_MIXINS`, or
../horizon-mixins). Everything generated goes under target/. Runs on Linux, macOS and Windows.
"""
import argparse
import contextlib
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMPAT = json.loads((ROOT / "compatibility.json").read_text())
PIN = COMPAT["horizon_revision"]
MIXINS_PIN = COMPAT["horizon_mixins_revision"]
BUNDLE = ROOT / "mixins" / "shulker_peek"
MANIFEST = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text())
MOD_ID, VERSION = MANIFEST["mod"]["id"], MANIFEST["mod"]["version"]
TOOLCHAIN = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
EXE = ".exe" if os.name == "nt" else ""
# The runtime's tests (game::shulker_peek::tests) and, with --with tests/native, the parity tests.
TESTS = MOD_ID


def digest(data):
    return hashlib.sha256(data).hexdigest()


def log(*args):
    print(*args, file=sys.stderr, flush=True)


@contextlib.contextmanager
def lock(path):
    """An exclusive lock on `path` across processes (fcntl on POSIX, msvcrt on Windows)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+b") as handle:
        if os.name == "nt":
            import msvcrt
            handle.seek(0)
            while True:
                try:
                    msvcrt.locking(handle.fileno(), msvcrt.LK_LOCK, 1)
                    break
                except OSError:
                    time.sleep(0.5)
            try:
                yield
            finally:
                handle.seek(0)
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
        else:
            import fcntl
            fcntl.flock(handle, fcntl.LOCK_EX)
            yield


def export(horizon, stages):
    """The pinned commit's committed files, once: target/mixins/original/<revision>/. Links are left out, as the
    launcher leaves them out of GitHub's archive (horizon-mixins refuses them; nothing needs them to build)."""
    if not (horizon / ".git").exists():
        raise ValueError(f"--horizon must be a Git clone of Horizon: {horizon}")
    found = subprocess.run(["git", "-C", str(horizon), "cat-file", "-e", PIN + "^{commit}"], capture_output=True)
    if found.returncode:
        raise ValueError(f"{horizon} has no commit {PIN}: fetch it (git -C {horizon} fetch origin {PIN})")
    out = stages / "original" / PIN
    marker = out / ".shulker-peek-export.json"
    if marker.is_file():
        files = json.loads(marker.read_text())["files"]
        for path, expected in files.items():
            if digest((out / path).read_bytes()) != expected:
                raise ValueError(f"the exported source was edited: {path}; remove {out} to export it again")
        return out
    partial = stages / "original" / f".{PIN}.partial"
    shutil.rmtree(partial, ignore_errors=True)
    partial.mkdir(parents=True)
    try:
        # The committed bytes, as GitHub's archive (which the launcher builds) has them: no CRLF conversion
        # from a Windows clone's core.autocrlf.
        process = subprocess.Popen(["git", "-c", "core.autocrlf=false", "-C", str(horizon), "archive",
                                    "--format=tar", PIN], stdout=subprocess.PIPE)
        try:
            with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
                for member in archive:
                    if member.isfile() or member.isdir():
                        archive.extract(member, partial, filter="data")
        finally:
            process.stdout.close()
            code = process.wait()
        if code:
            raise ValueError(f"git archive exited {code}")
        for path, expected in COMPAT["source_sha256"].items():
            if digest((partial / path).read_bytes()) != expected:
                raise ValueError(f"pinned source hash mismatch: {path}")
        (partial / ".horizon-revision").write_text(PIN + "\n")
        files = {p.relative_to(partial).as_posix(): digest(p.read_bytes())
                 for p in sorted(partial.rglob("*")) if p.is_file()}
        marker = partial / ".shulker-peek-export.json"
        marker.write_text(json.dumps({"horizon_revision": PIN, "files": files}, sort_keys=True, indent=2) + "\n")
        shutil.rmtree(out, ignore_errors=True)
        os.replace(partial, out)
    except BaseException:
        shutil.rmtree(partial, ignore_errors=True)
        raise
    return out


def mixins_root(given):
    root = Path(given or os.environ.get("HORIZON_MIXINS") or ROOT.parent / "horizon-mixins").resolve()
    revision = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
    if revision != MIXINS_PIN:
        raise ValueError(f"horizon-mixins at {root} must be at {MIXINS_PIN}, it is at {revision}")
    clean = subprocess.run(["git", "-C", str(root), "diff", "--quiet", "HEAD", "--", "Cargo.toml", "Cargo.lock",
                            "crates"])
    if clean.returncode:
        raise ValueError(f"horizon-mixins at {root} has local changes to its engine")
    return root


def mixins_cli(root):
    """horizon-mixins, built with its own pinned toolchain and lockfile."""
    subprocess.run(["cargo", "build", "--locked", "--release", "-p", "horizon-mixins-cli"], cwd=root, check=True,
                   stdout=sys.stderr)
    return root / "target" / "release" / f"horizon-mixins{EXE}"


def bundles(extra, stages):
    """The bundles to apply, copied into one folder: this mod's, and each `--with` bundle pinned to the same
    commit, as the launcher pins a hook-point mod to the commit it builds on."""
    if not extra:
        return ROOT / "mixins", MOD_ID
    found = [BUNDLE]
    for path in extra:
        path = Path(path).resolve()
        found += [p.parent for p in sorted(path.glob("**/horizon-mixin.toml"))]
    ids = []
    key_dir = stages / "bundles"
    shutil.rmtree(key_dir, ignore_errors=True)
    for bundle in found:
        manifest = tomllib.loads((bundle / "horizon-mixin.toml").read_text())
        bundle_id = manifest["mod"]["id"]
        if bundle_id in ids:
            raise ValueError(f"two bundles have the id {bundle_id}")
        ids.append(bundle_id)
        copy = key_dir / bundle_id
        shutil.copytree(bundle, copy)
        toml = copy / "horizon-mixin.toml"
        toml.write_text(re.sub(r'horizon_revision = "[0-9a-f]{40}"', f'horizon_revision = "{PIN}"',
                               toml.read_text()))
    return key_dir, "+".join(ids)


def sync(fresh, out):
    """Makes `out` the same as `fresh`, replacing only files whose bytes differ and removing the rest, so Cargo
    (which goes by mtimes) rebuilds only what changed: `rsync --checksum --delete`, as the launcher and the
    registry's tool keep their stage. Returns whether anything changed."""
    changed = False
    wanted = {p.relative_to(fresh) for p in fresh.rglob("*") if p.is_file()}
    for relative in sorted(wanted):
        source, target = fresh / relative, out / relative
        if target.is_file() and target.read_bytes() == source.read_bytes():
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        changed = True
    if out.is_dir():
        for path in sorted(out.rglob("*"), reverse=True):
            if path.is_file() and path.relative_to(out) not in wanted:
                path.unlink()
                changed = True
            elif path.is_dir() and not any(path.iterdir()):
                path.rmdir()
    return changed


def stage(horizon, mixins, extra=()):
    """Export, plan and apply: target/mixins/modded[-<ids>]/. Unchanged files keep their bytes and mtimes.
    horizon-mixins applies into a fresh folder (it replaces an existing stage atomically only on Linux), which is
    then synced into the stage."""
    stages = ROOT / "target" / "mixins"
    original = export(horizon, stages)
    cli = mixins_cli(mixins)
    mods, key = bundles(extra, stages)
    suffix = "" if key == MOD_ID else "-" + digest(key.encode())[:12]
    output = stages / f"modded{suffix}"
    plan = stages / f"plan{suffix}.json"
    base = ["--workspace", str(original), "--mods", str(mods)]
    planned = subprocess.run([str(cli), "plan", *base, "--out", str(plan)], capture_output=True, text=True)
    if planned.returncode:
        raise ValueError(f"horizon-mixins plan failed:\n{planned.stderr}")
    # Applied in the system's temporary folder: on Windows an editor or a scanner watching the project can hold
    # a just-written file and fail horizon-mixins' final rename with E_IO.
    scratch = Path(tempfile.mkdtemp(prefix="shulker-peek-apply-"))
    fresh = scratch / "stage"
    try:
        for attempt in range(3):
            applied = subprocess.run([str(cli), "apply", *base, "--out", str(fresh)], capture_output=True,
                                     text=True)
            if not applied.returncode or '"E_IO"' not in applied.stderr or attempt == 2:
                break
            shutil.rmtree(fresh, ignore_errors=True)
            time.sleep(2)
        if applied.returncode:
            raise ValueError(f"horizon-mixins apply failed:\n{applied.stderr}")
        report = json.loads(applied.stdout)
        changed = sync(fresh, output)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
    return {"original": original, "stage": output, "plan": plan, "mods": key, "stage_changed": changed,
            "injections": report["injections"], "modules": report["modules"]}


def cargo(action, staged, target, features):
    command = ["cargo", action, "--locked", "-p", "horizon-client", "--bin", "horizon-client", *features]
    if action == "test":
        # The sync module's tests are in the game's library, the rest in its binary.
        command.insert(5, "--lib")
    if action == "build":
        command += ["--bin", "horizon-ctl", "--profile", "fast"]
    if action == "test":
        command += ["--", TESTS, "--nocapture"]
    env = dict(os.environ, RUSTUP_TOOLCHAIN=TOOLCHAIN, CARGO_TARGET_DIR=str(target))
    with lock(target.parent / (target.name + ".lock")):
        started = time.monotonic()
        result = subprocess.run(command, cwd=staged, env=env, capture_output=True, text=True)
    log_file = ROOT / "target" / f"horizon-{action}.log"
    log_file.write_text(result.stdout + result.stderr)
    return {"cargo_command": command, "cargo_exit": result.returncode, "rust": TOOLCHAIN,
            "cargo_seconds": round(time.monotonic() - started, 1), "cargo_log": str(log_file)}, result


def package():
    """A deterministic .hnmod: horizon-native.json and the bundle's .rs and .toml files, as the registry's own
    native mods are packaged (fixed timestamps and modes, sorted names)."""
    pinned = MANIFEST.get("compatibility", {}).get("horizon_revision")
    if pinned != PIN:
        raise ValueError(f"horizon-mixin.toml pins {pinned}, compatibility.json {PIN}")
    manifest = {"schema": 1, "id": MOD_ID, "version": VERSION, "game": COMPAT["game"], "horizon": PIN,
                "mixins": MIXINS_PIN}
    files = {"horizon-native.json": (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()}
    for path in sorted(BUNDLE.iterdir()):
        if path.suffix in (".rs", ".toml"):
            files[f"mixins/{MOD_ID}/{path.name}"] = path.read_bytes().replace(b"\r\n", b"\n")
    out = ROOT / "target" / f"{MOD_ID}-{VERSION}.hnmod"
    out.parent.mkdir(exist_ok=True)
    with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, content in sorted(files.items()):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, content)
    return {"file": str(out), "size": out.stat().st_size, "sha256": digest(out.read_bytes()),
            "entries": sorted(files)}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("action", choices=["stage", "check", "test", "build", "package"])
    parser.add_argument("--horizon", type=Path, default=ROOT.parent / "horizon")
    parser.add_argument("--mixins", type=Path)
    parser.add_argument("--with", dest="extra", action="append", default=[], metavar="DIR")
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target" / "horizon-cargo")
    parser.add_argument("--no-mods-feature", action="store_true",
                        help="leave out horizon-client/mods (the launcher builds with it)")
    parser.add_argument("--no-default-features", action="store_true", help="no audio and controllers")
    args = parser.parse_args()
    if args.action == "package":
        print(json.dumps(package(), indent=2))
        return 0
    horizon = args.horizon.resolve()
    target = args.target_dir.resolve()
    if target.is_relative_to(horizon):
        parser.error("the Cargo target must be outside the Horizon clone")
    started = time.monotonic()
    with lock(ROOT / "target" / "stage.lock"):
        staged = stage(horizon, mixins_root(args.mixins), args.extra)
    report = {"horizon_revision": PIN, "horizon_mixins_revision": MIXINS_PIN,
              "staging_seconds": round(time.monotonic() - started, 1),
              **{k: str(v) if isinstance(v, Path) else v for k, v in staged.items()}}
    if args.action != "stage":
        features = [] if args.no_mods_feature else ["--features", "horizon-client/mods"]
        if args.no_default_features:
            features.append("--no-default-features")
        result_report, result = cargo(args.action, staged["stage"], target, features)
        report.update(result_report)
        if result.returncode:
            print(result.stderr[-12000:], file=sys.stderr)
            print(json.dumps(report, indent=2), file=sys.stderr)
            return result.returncode
        if args.action == "test":
            report["tests_passed"] = sum(int(n) for n in re.findall(r"test result: ok\. (\d+) passed", result.stdout))
            if not report["tests_passed"]:
                print(result.stdout[-4000:], file=sys.stderr)
                return 1
        if args.action == "build":
            report["binaries"] = [str(target / "fast" / f"{b}{EXE}") for b in ("horizon-client", "horizon-ctl")]
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        print(f"ShulkerPeek: {error}", file=sys.stderr)
        raise SystemExit(1)

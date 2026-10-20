#!/usr/bin/env python3
"""VS Code A/B check in a disposable profile and private Xvfb display."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

root = Path(sys.argv[1])
with tempfile.TemporaryDirectory(prefix="lyapunov-vscode-") as tmp:
    base = Path(tmp)
    for name in ("bin", "run", "ext", "user", "data", "A", "B"):
        (base / name).mkdir()
    (base / "run").chmod(0o700)
    wrapper = base / "bin/code"
    wrapper.write_text(
        '#!/usr/bin/env bash\n'
        'exec "$LYAPUNOV_TEST_CODE_BIN" --user-data-dir "$LYAPUNOV_TEST_USER_DIR" '
        '--extensions-dir "$LYAPUNOV_TEST_EXT_DIR" --ozone-platform=x11 "$@"\n'
    )
    wrapper.chmod(0o700)
    env = os.environ.copy()
    env.update({
        "PATH": f"{base / 'bin'}:{env['PATH']}",
        "LYAPUNOV_TEST_USER_DIR": str(base / "user"),
        "LYAPUNOV_TEST_EXT_DIR": str(base / "ext"),
        "XDG_RUNTIME_DIR": str(base / "run"),
        "XDG_DATA_HOME": str(base / "data"),
        "XDG_SESSION_TYPE": "x11",
    })
    env.pop("WAYLAND_DISPLAY", None)
    core_env = env.copy()
    core_env.pop("DBUS_SESSION_BUS_ADDRESS", None)
    core = root / "target/debug/lyapunov"

    def lyapunov(*args, input=None):
        out = subprocess.check_output([str(core), *args], input=input, env=core_env, text=True)
        return json.loads(out.splitlines()[-1])

    def folders():
        result = []
        for marker in (base / "run/lyapunov-vscode").glob("*.json"):
            try:
                result.append(json.loads(marker.read_text())["folder"])
            except (OSError, ValueError, KeyError):
                pass
        return sorted(result)

    def wait_for(expected, timeout=25):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if folders() == expected:
                return
            time.sleep(0.3)
        raise AssertionError(f"expected project markers {expected}, found {folders()}")

    def cleanup():
        # Every matched process belongs to this unique, disposable VS Code
        # profile. Never inspect or signal the user's normal VS Code process.
        listing = subprocess.check_output(["ps", "-eo", "pid=,args="], text=True)
        for line in listing.splitlines():
            pid_text, _, args = line.strip().partition(" ")
            if str(base / "user") not in args:
                continue
            pid = int(pid_text)
            if pid != os.getpid():
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass

    try:
        subprocess.run([str(root / "scripts/install-vscode-extension.sh")], env=env, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(["code", "--new-window", str(base / "A"), "--disable-gpu",
                        "--skip-welcome", "--disable-telemetry"], env=env, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        wait_for([str(base / "A")])
        draft = {"name": "Project B", "apps": [{"name": "Visual Studio Code",
                 "desktop": "code.desktop", "class": "Code", "workspace": str(base / "B"), "windows": []}]}
        saved = lyapunov("save", "--json", input=json.dumps(draft))
        card_file = base / "card.json"
        card_file.write_text(json.dumps(saved))
        preview = lyapunov("import", str(card_file), "--dry-run", "--json")
        assert preview["cards"][0]["workspaces"] == [str(base / "B")]
        assert lyapunov("plan", "Project B", "--json")["steps"][0]["kind"] == "launch"
        first = lyapunov("resume", "Project B", "--json", "--timeout", "25")
        assert first["outcomes"][0]["status"] == "app-launched", first
        wait_for(sorted([str(base / "A"), str(base / "B")]))
        assert lyapunov("plan", "Project B", "--json")["steps"][0]["kind"] == "running"
        second = lyapunov("resume", "Project B", "--json", "--timeout", "5")
        assert second["outcomes"][0]["status"] == "left-open", second
        assert folders() == sorted([str(base / "A"), str(base / "B")])
        print("VS Code A/B: A stayed open, B opened once, second resume reused B")
    finally:
        cleanup()

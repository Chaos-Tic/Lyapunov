#!/usr/bin/env python3
"""End-to-end check of capture and resume in a real (isolated) desktop session.

Run by the scripts/test-*.sh wrappers, which start a private compositor or X
server, a private D-Bus session and private data folders. It never touches the
desktop you are logged into.

Environment:
  LYAPUNOV        path to the lyapunov executable
  LYA_TEST_CMD    command that opens one test window (split on spaces)
  LYA_TEST_ID     desktop entry ID that starts the same application
  LYA_EVIDENCE    folder for the JSON evidence of each step (optional)
"""
import json
import os
import shlex
import subprocess
import sys
import time

LYA = os.environ["LYAPUNOV"]
CMD = shlex.split(os.environ["LYA_TEST_CMD"])
APP_ID = os.environ["LYA_TEST_ID"]
EVIDENCE = os.environ.get("LYA_EVIDENCE")
TOL = 16
step_no = 0


def run(*args, stdin=None, check=True):
    p = subprocess.run([LYA, *args, "--json"], input=stdin, capture_output=True, text=True, timeout=120,
                       env={**os.environ, "LYAPUNOV_DEBUG": "1"})
    if EVIDENCE and p.stderr:
        with open(os.path.join(EVIDENCE, "debug.log"), "a") as f:
            f.write(f"$ lyapunov {' '.join(args)}\n{p.stderr}")
    lines = [l for l in p.stdout.splitlines() if l.strip()]
    if not lines:
        raise SystemExit(f"no output from {args}: {p.stderr}")
    out = json.loads(lines[-1])
    if check and (p.returncode or "error" in out):
        raise SystemExit(f"{args} failed: {out} {p.stderr}")
    return out


def record(name, data):
    global step_no
    step_no += 1
    if EVIDENCE:
        with open(os.path.join(EVIDENCE, f"{step_no:02d}-{name}.json"), "w") as f:
            json.dump(data, f, indent=2)


def ok(cond, msg):
    if not cond:
        raise SystemExit(f"FAIL: {msg}")
    print(f"  ok  {msg}")


def wait_for(pred, timeout=20, what="condition"):
    end = time.time() + timeout
    while time.time() < end:
        v = pred()
        if v:
            return v
        time.sleep(0.3)
    raise SystemExit(f"FAIL: timed out waiting for {what}")


def our_app(scan):
    for c in scan["apps"]:
        if c["app"].get("desktop") == APP_ID:
            return c
    return None


def close(a, b):
    return all(abs(a[k] - b[k]) <= TOL for k in ("x", "y", "width", "height"))


def windows_now():
    scan = run("scan")
    record("scan-now", scan)
    c = our_app(scan)
    return c["app"]["windows"] if c else []


doctor = run("doctor")
record("doctor", doctor)
caps = doctor["capabilities"]
backend = doctor["backend"]
print(f"backend {backend}: {caps}")
ok(caps["list"] and caps["geometry"], "the backend lists and moves windows")

proc = subprocess.Popen(CMD, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
scan = wait_for(lambda: our_app(run("scan")), what="the test window")
record("scan", run("scan"))
ok(scan["launchable"], f"the test window is recognised as {APP_ID}")
ok(scan["titles"] and all(isinstance(t, str) for t in scan["titles"]), "titles are shown for the preview")

card = run("capture", "E2E check")
record("capture", card)
apps = [a for a in card["apps"] if a.get("desktop") == APP_ID]
ok(len(apps) == 1 and len(apps[0]["windows"]) == 1, "capture keeps one window of the test application")
ok("title" not in json.dumps(card), "no window title is stored in the card")
cid = card["id"]

plan = run("plan", cid)
record("plan", plan)
ok(any(s["kind"] == "running" for s in plan["steps"]), "the plan reuses the open window")

slot = apps[0]["windows"][0]
area = slot.get("output_rect") or {"x": 0, "y": 0, "width": 1024, "height": 768}
target = {"x": area["x"] + 61, "y": area["y"] + 73, "width": 520, "height": 340}


def set_slot(**fields):
    c = run("show", cid)
    for a in c["apps"]:
        if a.get("desktop") == APP_ID:
            a["windows"][0].update(fields)
    c["apps"] = [a for a in c["apps"] if a.get("desktop") == APP_ID]
    c["places"] = []
    saved = run("save", stdin=json.dumps(c))
    ok(saved["id"] == cid, "the edited card keeps its identity")
    return saved


set_slot(rect=target, state="normal", floating=True)
report = run("resume", cid, "--timeout", "15")
record("resume-open", report)
o = report["outcomes"][0]
ok(o["status"] == "window-found" and o["placed"] == 1, f"resume finds the open window and places it ({o})")
w = wait_for(windows_now, what="window list")[0]
ok(close(w["rect"], target), f"the window is where the card says ({w['rect']} vs {target})")

proc.terminate()
proc.wait(timeout=10)
wait_for(lambda: not windows_now(), what="the test window to close")
target2 = {"x": area["x"] + 140, "y": area["y"] + 95, "width": 480, "height": 300}
set_slot(rect=target2)
plan = run("plan", cid)
ok(plan["steps"][0]["kind"] == "launch", "the plan starts a closed application")
report = run("resume", cid, "--timeout", "20")
record("resume-launch", report)
o = report["outcomes"][0]
ok(o["status"] == "app-launched" and o["placed"] == 1, f"resume starts the application and places its window ({o})")
w = wait_for(windows_now, what="the started window")[0]
ok(close(w["rect"], target2), f"the started window is in place ({w['rect']} vs {target2})")

ghost = {"x": area["x"] + area["width"] + 4000, "y": 300, "width": 600, "height": 400}
set_slot(rect=ghost, output="GHOST-9", output_rect={"x": ghost["x"] - 100, "y": 0, "width": 1920, "height": 1080})
report = run("resume", cid)
record("resume-missing-screen", report)
w = windows_now()[0]
r = w["rect"]
inside = (r["x"] >= area["x"] - TOL and r["y"] >= area["y"] - TOL
          and r["x"] + r["width"] <= area["x"] + area["width"] + TOL
          and r["y"] + r["height"] <= area["y"] + area["height"] + TOL)
ok(inside, f"a window from a missing screen lands fully on a present one ({r} in {area})")
ok(any("not connected" in n for n in report["outcomes"][0].get("notes", [])), "the report says the screen is missing")

if caps["desktops"] and not caps["tiling"]:
    second = os.environ.get("LYA_SECOND_DESKTOP")
    if second:
        set_slot(rect=target, output=slot.get("output"), output_rect=slot.get("output_rect"),
                 desktop={"id": second, "name": "Second", "index": 1})
        report = run("resume", cid)
        record("resume-desktop", report)
        snap_scan = run("scan")
        d = our_app(snap_scan)["app"]["windows"][0].get("desktop", {})
        ok(d.get("id") == second, f"the window moved to the saved desktop ({d})")

if caps["states"] and backend not in ("sway", "i3", "hyprland"):
    set_slot(rect=target, state="maximized", desktop=slot.get("desktop"))
    report = run("resume", cid)
    record("resume-maximized", report)
    st = wait_for(lambda: windows_now()[0]["state"] == "maximized" and "maximized", timeout=5, what="maximized state")
    ok(st == "maximized", "the window is maximized again")
    set_slot(rect=target, state="normal")
    run("resume", cid)
    w = wait_for(lambda: [x for x in windows_now() if x["state"] == "normal"], timeout=5, what="normal state")[0]
    ok(close(w["rect"], target), f"un-maximized, the window returns to its saved place ({w['rect']})")

deleted = run("delete", cid)
ok(deleted["deleted"] == cid, "the card is deleted")
ok(windows_now(), "deleting the card closed nothing")
subprocess.run(["pkill", "-f", CMD[0]], check=False)
print(f"PASS: {backend}")

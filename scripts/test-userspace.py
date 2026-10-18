#!/usr/bin/env python3
"""QEMU/OVMF serial regression for Nova's first Ring 3 milestone."""
import os
import pathlib
import re
import shutil
import subprocess
import sys
import time
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = tomllib.loads((ROOT / "kernel/Cargo.toml").read_text())["package"]["version"]
BUILD = ROOT / "build"
QEMU = os.environ.get("NOVA_QEMU", "qemu-system-x86_64")
CODE = os.environ.get("OVMF_CODE", "/usr/share/OVMF/OVMF_CODE_4M.fd")
VARS = os.environ.get("OVMF_VARS", CODE.replace("CODE", "VARS"))
for path in (CODE, VARS):
    if not pathlib.Path(path).is_file():
        sys.exit(f"OVMF image missing: {path}")
if shutil.which(QEMU) is None:
    sys.exit(f"QEMU missing: {QEMU}")
if "--no-build" not in sys.argv:
    subprocess.run([str(ROOT / "scripts/build.sh")], cwd=ROOT, check=True, env={**os.environ, "NOVA_MONITOR": "1"})
shutil.copy(VARS, BUILD / "OVMF_VARS.fd")
log = BUILD / "test-userspace.log"
log.write_text("")
cmd = [QEMU]
if os.environ.get("NOVA_QEMU_DATADIR"):
    cmd += ["-L", os.environ["NOVA_QEMU_DATADIR"]]
cmd += ["-machine", "q35", "-m", os.environ.get("NOVA_RAM", "256M"), "-cpu", "qemu64", "-smp", "1",
        "-drive", f"if=pflash,format=raw,readonly=on,file={CODE}",
        "-drive", f"if=pflash,format=raw,file={BUILD / 'OVMF_VARS.fd'}",
        "-drive", f"format=raw,file=fat:rw:{BUILD / 'esp'}",
        "-drive", f"if=none,id=nova-system,format=raw,file={BUILD / 'nova-system.img'}",
        "-device", "virtio-blk-pci,drive=nova-system,disable-modern=on",
        "-device", "VGA", "-display", "none", "-serial", f"file:{log}", "-monitor", "stdio", "-no-reboot"]
env = os.environ.copy()
env["TMPDIR"] = str(BUILD)
proc = subprocess.Popen(cmd, cwd=ROOT, env=env, stdin=subprocess.PIPE,
                        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

def wait_for(marker, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        content = log.read_text(errors="replace")
        if marker in content:
            return content
        if proc.poll() is not None:
            raise RuntimeError("QEMU exited: " + proc.stderr.read().decode(errors="replace"))
        time.sleep(0.05)
    raise RuntimeError(f"Timed out waiting for {marker!r}; see {log}")

def send(line):
    for char in line + "\n":
        key = {" ": "spc", "\n": "ret"}.get(char, char)
        proc.stdin.write(f"sendkey {key}\n".encode())
        proc.stdin.flush()
        time.sleep(0.06)

try:
    wait_for("nova> ")
    screenshot_taken = False
    for command in ("cpu", "mem", "uptime", "run hello", "run hello", "run exit42",
                    "run yieldtest", "run invalidptr", "run invalidsys", "run badmem",
                    "run badopcode", "run badpriv", "run badcodewrite", "run badstackexec", "run badkernel", "run hello", "mem"):
        before = log.read_text(errors="replace").count("nova> ")
        send(command)
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if log.read_text(errors="replace").count("nova> ") > before:
                break
            time.sleep(0.05)
        else:
            raise RuntimeError(f"Command did not return to shell: {command}")
        if command == "run hello" and not screenshot_taken:
            proc.stdin.write(f"screendump {BUILD / 'nova-hello.ppm'}\n".encode())
            proc.stdin.flush()
            screenshot_taken = True
    proc.stdin.write(f"screendump {BUILD / 'nova-0.3.ppm'}\n".encode())
    proc.stdin.flush()
    time.sleep(0.2)
    content = log.read_text(errors="replace")
    expected = (f"Nova OS {VERSION}", "Vendor:", "Physical:", "Usable:", "Hello from Nova userspace!", "PID: 1",
                "PID: 2", "PID: 13", "Process 3 exited with code 42", "Before yield",
                "After yield", "Invalid pointer rejected", "Invalid syscall rejected",
                "Exception: PAGE FAULT", "Exception: INVALID OPCODE",
                "Exception: GENERAL PROTECTION FAULT", "Process 10 faulted.", "Process 11 faulted.",
                "Process 12 faulted.", "Process 13 exited with code 0")
    for marker in expected:
        if marker not in content:
            raise AssertionError(f"Missing {marker!r}; see {log}")
    if "NOVA KERNEL PANIC" in content or "NOVA KERNEL EXCEPTION" in content:
        raise AssertionError(f"Kernel failure; see {log}")
    if not re.search(r"\d{2}:\d{2}:\d{2}\.\d{3}", content):
        raise AssertionError("uptime command did not report time")
    used = [int(x) for x in re.findall(r"Used: (\d+) KiB", content)]
    if len(used) != 2 or used[0] != used[1]:
        raise AssertionError(f"Physical frame count did not recover: {used}")
    print(f"NOVA_TEST_SUCCESS userspace; physical used KiB {used[0]} -> {used[1]}")
finally:
    proc.terminate()
    try: proc.wait(timeout=3)
    except subprocess.TimeoutExpired: proc.kill()

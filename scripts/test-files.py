#!/usr/bin/env python3
"""QEMU/OVMF regression for Nova FAT32 and file syscalls."""
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
    subprocess.run([str(ROOT / "scripts/build.sh")], cwd=ROOT, check=True, env={**os.environ, "NOVA_MONITOR": "0"})
shutil.copy(VARS, BUILD / "OVMF_VARS.fd")
log = BUILD / ("test-files-persistence.log" if "--verify-persistence" in sys.argv else "test-files.log")
log.write_text("")
cmd = [QEMU]
if os.environ.get("NOVA_QEMU_DATADIR"):
    cmd += ["-L", os.environ["NOVA_QEMU_DATADIR"]]
cmd += ["-machine", "q35", "-m", os.environ.get("NOVA_RAM", "256M"), "-cpu", "qemu64", "-smp", os.environ.get("NOVA_SMP", "1"),
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
        key = {" ": "spc", "\n": "ret", "/": "slash", ".": "dot"}.get(char, "shift-" + char.lower() if char.isupper() else char)
        proc.stdin.write(f"sendkey {key}\n".encode())
        proc.stdin.flush()
        time.sleep(0.06)

try:
    wait_for("nova:/$ ")
    commands = ("cat /home/filetest.txt",) if "--verify-persistence" in sys.argv else ("ls /", "ls /bin", "cat /etc/version", "ls /home/../bin", "cd /bin", "pwd", "ls", "./hello", "./filetest", "./badmem", "./badopcode", "./hello", "cd /", "cat /home/filetest.txt")
    for command in commands:
        before = log.read_text(errors="replace").count("$ ")
        send(command)
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            if log.read_text(errors="replace").count("$ ") > before:
                break
            time.sleep(0.05)
        else:
            raise RuntimeError(f"Command did not return to shell: {command}")
        if command in ("ls /bin", "./filetest"):
            label = "directory" if command == "ls /bin" else "filetest"
            proc.stdin.write(f"screendump {BUILD / f'nova-{label}.ppm'}\n".encode())
            proc.stdin.flush()
    proc.stdin.write(f"screendump {BUILD / 'nova-files.ppm'}\n".encode())
    proc.stdin.flush()
    time.sleep(0.3)
    content=log.read_text(errors="replace")
    expected = ("Userspace wrote this", "FAT32 mounted at /") if "--verify-persistence" in sys.argv else ("FAT32 mounted at /", "filetest", f"Nova OS {VERSION}", "/bin", "Hello from Nova userspace!", "Opening /etc/version...", "File test passed.", "fault vector 14", "fault vector 6", "Userspace wrote this")
    for marker in expected:
        if marker not in content: raise AssertionError(f"Missing {marker!r} in {log}")
    if "--verify-persistence" not in sys.argv:
        for unwanted in ("File write failed", "File verify failed", "EOF failed", "User pointer validation failed", "touch:", "write:", "mkdir:", "rm:", "NOVA KERNEL PANIC"):
            if unwanted in content: raise AssertionError(f"Unexpected {unwanted!r} in {log}")
    print("NOVA_TEST_SUCCESS files, Ring 3, faults")
finally:
    proc.terminate()
    try: proc.wait(timeout=3)
    except subprocess.TimeoutExpired: proc.kill()

#!/usr/bin/env python3
"""QEMU/OVMF regression for Nova userspace init, nsh and independent C ELF tools."""
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
log = BUILD / "test-init.log"
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
        key = {" ": "spc", "\n": "ret", "/": "slash", ".": "dot", "-": "minus", '&': "shift-7", '"': "shift-apostrophe"}.get(char, "shift-" + char.lower() if char.isupper() else char)
        proc.stdin.write(f"sendkey {key}\n".encode())
        proc.stdin.flush()
        time.sleep(0.06)

try:
    wait_for("nova:/$ ")
    proc.stdin.write(f"screendump {BUILD / 'nova-boot.ppm'}\n".encode())
    proc.stdin.flush()
    commands = ("pwd", "ls /", "ls /dev", "ls /proc", "cat /proc/uptime", "sysinfo", "ps", "uptime", "dmesg", "env", "devtest", "fbtest", "cat /etc/version", "hello-c", "malloctest", "memtest", "spawntest-c", "schedtest", "threadtest", "pipetest", "msgtest", "orphantest", "killtest", 'args one two "hello nova"', "cd /bin", "pwd", "./pwd", 'echo "Nova works"', "badmem", "badopcode", "invalidptr", "invalidsys", "syscheck", "hello", "hello-c", "hello-c", "hello-c")
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
        if command == "cat /etc/version":
            proc.stdin.write(f"screendump {BUILD / 'nova-shell.ppm'}\n".encode())
            proc.stdin.flush()
    before = log.read_text(errors="replace").count("$ ")
    send("sleeper 5000 &")
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline and log.read_text(errors="replace").count("$ ") <= before:
        time.sleep(0.05)
    spawned = re.findall(r"\[PID (\d+) running\]", log.read_text(errors="replace"))
    if not spawned: raise AssertionError("Background process did not start")
    before = log.read_text(errors="replace").count("$ ")
    send("kill " + spawned[-1])
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline and log.read_text(errors="replace").count("$ ") <= before:
        time.sleep(0.05)
    proc.stdin.write(f"screendump {BUILD / 'nova-init.ppm'}\n".encode())
    proc.stdin.flush()
    time.sleep(0.3)
    content=log.read_text(errors="replace")
    expected = ("Nova OS 0.6.0", "launching /system/init as PID 1", "NovaShell (Ring 3)", "null", "zero", "meminfo", "UptimeMilliseconds:", "Nova system information", "PID 1", "Uptime:", "PATH=/bin", "Device test passed. Framebuffer", "Framebuffer test passed.", "Hello from Nova C userspace!", "Malloc test passed.", "Memory test passed. Invalid access isolated.", "C spawn test passed:", "Scheduler test passed.", "Userspace thread test passed.", "Pipe test passed.", "Message channel test passed.", "Orphan test spawned PID", "orphan PID", "Kill test passed.", "Killed PID", "argc = 4", "argv[3] = hello nova", "Nova works", "fault vector 14", "fault vector 6", "Invalid pointer rejected", "Invalid syscall rejected", "Nova stderr works.", "Syscall validation passed.", "Hello from Nova userspace!")
    for marker in expected:
        if marker not in content: raise AssertionError(f"Missing {marker!r} in {log}")
    cpu_lines = re.findall(r"(?m)^([AB][123])\r?$", content)
    if cpu_lines != ["A1", "B1", "A2", "B2", "A3", "B3"]:
        raise AssertionError(f"CPU-bound children did not interleave as expected: {cpu_lines}")
    if content.count("Hello from Nova C userspace!") != 4:
        raise AssertionError("Repeated C process execution did not complete four times")
    if "--verify-persistence" not in sys.argv:
        for unwanted in ("File write failed", "File verify failed", "EOF failed", "User pointer validation failed", "touch:", "write:", "mkdir:", "rm:", "NOVA KERNEL PANIC"):
            if unwanted in content: raise AssertionError(f"Unexpected {unwanted!r} in {log}")
    print("NOVA_TEST_SUCCESS init, nsh, libc, C ELF tools")
finally:
    proc.terminate()
    try: proc.wait(timeout=3)
    except subprocess.TimeoutExpired: proc.kill()

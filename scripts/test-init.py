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
        key = {" ": "spc", "\n": "ret", "/": "slash", ".": "dot", "-": "minus", '&': "shift-7", '"': "shift-apostrophe"}.get(char, "shift-" + char.lower() if char.isupper() else char)
        proc.stdin.write(f"sendkey {key}\n".encode())
        proc.stdin.flush()
        time.sleep(0.06)

try:
    wait_for("nova:/$ ")
    proc.stdin.write(f"screendump {BUILD / 'nova-boot.ppm'}\n".encode())
    proc.stdin.flush()
    commands = ("pwd", "ls /", "ls /dev", "ls /proc", "cat /proc/uptime", "sysinfo", "cpuinfo", "ps", "uptime", "dmesg", "env", "devtest", "fbtest", "cat /etc/version", "hello-c", "malloctest", "memtest", "spawntest-c", "schedtest", "threadtest", "pipetest", "msgtest", "gsattack", "orphantest", "killtest", 'args one two "hello nova"', "cd /bin", "pwd", "./pwd", 'echo "Nova works"', "badmem", "badopcode", "invalidptr", "invalidsys", "syscheck", "hello", "hello-c", "hello-c", "hello-c")
    for command in commands:
        before = log.read_text(errors="replace").count("$ ")
        send(command)
        if command == "devtest":
            wait_for("[devtest] keyboard lease active")
            proc.stdin.write(b"sendkey x\n")
            proc.stdin.flush()
        if command == "fbtest":
            wait_for("[fbtest] framebuffer lease active")
            shot = BUILD / "nova-fb-lease.ppm"
            shot.unlink(missing_ok=True)
            proc.stdin.write(f"screendump {shot}\n".encode())
            proc.stdin.flush()
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                ppm = shot.read_bytes() if shot.is_file() else b""
                header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", ppm)
                if header:
                    width, height = int(header[1]), int(header[2])
                    pixels = ppm[header.end():]
                    if len(pixels) == width * height * 3:
                        break
                time.sleep(0.02)
            else:
                raise AssertionError("QEMU framebuffer screenshot was not completed")
            # The full framebuffer is blue. Text drawn by the kernel terminal
            # during the lease would introduce light-gray glyph pixels.
            if any(pixels[(y * width + x) * 3:(y * width + x) * 3 + 3] != b"\x24\x5c\xa8"
                   for y in range(0, height, 13) for x in range(0, width - 64, 11)):
                raise AssertionError("Kernel terminal rendered over the framebuffer lease")
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
    key_section = content.split("[devtest] keyboard lease active", 1)[1].split("Device test passed", 1)[0]
    if "x" in key_section:
        raise AssertionError("Raw keyboard key leaked into the TTY during input lease")
    expected = ("Nova OS 0.6.0", "launching /system/init as PID 1", "NovaShell (Ring 3)", "null", "zero", "meminfo", "UptimeMilliseconds:", "Nova system information", "PID 1", "Uptime:", "PATH=/bin", "Device test passed. Framebuffer", "Framebuffer test passed.", "Hello from Nova C userspace!", "Malloc test passed.", "Memory test passed. Invalid access isolated.", "C spawn test passed:", "Scheduler test passed.", "Userspace thread test passed.", "Pipe test passed.", "Message channel test passed.", "GS syscall and timer entry test passed.", "Orphan test spawned PID", "orphan PID", "Kill test passed.", "Killed PID", "argc = 4", "argv[3] = hello nova", "Nova works", "fault vector 14", "fault vector 6", "Invalid pointer rejected", "Invalid syscall rejected", "Nova stderr works.", "Syscall validation passed.", "Hello from Nova userspace!")
    for marker in expected:
        if marker not in content: raise AssertionError(f"Missing {marker!r} in {log}")
    requested_cpus = int(os.environ.get("NOVA_SMP", "1"))
    if f"DiscoveredCPUs: {requested_cpus}" not in content:
        raise AssertionError("MADT processor count missing from cpuinfo")
    if f"SchedulerCPUs: {min(requested_cpus, 8)}" not in content or "UserSchedulerCPUs: 1" not in content:
        raise AssertionError("Kernel/user scheduler CPU counts missing from cpuinfo")
    if requested_cpus > 1:
        for apic_id in range(1, min(requested_cpus, 8)):
            marker = f"APIC {apic_id} entered Nova long-mode code"
            if marker not in content:
                raise AssertionError(f"Missing real AP startup acknowledgement: {marker}")
    if os.environ.get("NOVA_SMPTEST") == "1" and requested_cpus > 1:
        marker = f"SMP allocator stress passed on {min(requested_cpus, 8)-1} APs"
        if marker not in content:
            raise AssertionError(f"Missing completed cross-CPU allocator test: {marker}")
        marker = f"remote TLB IPI acknowledged on {min(requested_cpus, 8)-1} APs"
        if marker not in content:
            raise AssertionError(f"Missing remote TLB IPI acknowledgement: {marker}")
        marker = f"AP scheduler kernel workers passed on {min(requested_cpus, 8)-1} APs"
        if marker not in content:
            raise AssertionError(f"Missing AP scheduler worker test: {marker}")
        marker = f"concurrent TLB flush passed on {min(requested_cpus, 8)-1} APs"
        if marker not in content:
            raise AssertionError(f"Missing concurrent TLB flush test: {marker}")
    cpu_lines = re.findall(r"(?m)^([AB][123])\r?$", content)
    if (len(cpu_lines) != 6 or set(cpu_lines) != {"A1", "A2", "A3", "B1", "B2", "B3"}
            or not (cpu_lines.index("A1") < cpu_lines.index("A2") < cpu_lines.index("A3"))
            or not (cpu_lines.index("B1") < cpu_lines.index("B2") < cpu_lines.index("B3"))
            or not (cpu_lines.index("A1") < cpu_lines.index("B3")
                    and cpu_lines.index("B1") < cpu_lines.index("A3"))):
        raise AssertionError(f"CPU-bound children did not overlap correctly: {cpu_lines}")
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

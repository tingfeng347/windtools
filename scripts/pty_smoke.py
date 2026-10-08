#!/usr/bin/env python3
"""Exercise the real terminal event loop using an isolated data directory."""
import fcntl
import json
import os
from pathlib import Path
import select
import signal
import sqlite3
import struct
import subprocess
import tempfile
import termios
import time

root = Path(__file__).resolve().parents[1]
binary = root / "target/debug/windtools"

def start(directory, command="todo"):
    import pty
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    original = termios.tcgetattr(slave)
    def session():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        os.tcsetpgrp(slave, os.getpgrp())
    process = subprocess.Popen([str(binary), "--data-dir", str(directory), command], stdin=slave, stdout=slave, stderr=slave, preexec_fn=session, env={**os.environ, "TERM": "xterm-256color"})
    deadline = time.monotonic() + 20
    ready = bytearray()
    while b"\x1b[?2004h" not in ready:
        if process.poll() is not None or time.monotonic() > deadline:
            raise RuntimeError(f"TUI did not become ready: {bytes(ready)!r}")
        if select.select([master], [], [], 0.1)[0]:
            ready.extend(os.read(master, 65536))
    return process, master, slave, original

def drain(master, seconds=0.35):
    output = bytearray()
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                output.extend(os.read(master, 65536))
            except OSError:
                break
    return bytes(output)

def send(master, payload):
    os.write(master, payload)
    return drain(master)

with tempfile.TemporaryDirectory(prefix="windtools-smoke-") as temporary:
    directory = Path(temporary)
    process, master, slave, original = start(directory)
    capture = bytearray(drain(master))
    try:
        capture.extend(send(master, b"a"))
        capture.extend(send(master, b"\x1b[200~" + "中文事项测试".encode() + b"\x1b[201~"))
        capture.extend(send(master, b"\t"))
        capture.extend(send(master, "备注内容".encode()))
        capture.extend(send(master, b"\t+10m\x13"))
        connection = sqlite3.connect(directory / "windtools.sqlite3")
        row = connection.execute("SELECT title,note,reminder_state FROM tasks").fetchone()
        assert row == ("中文事项测试", "备注内容", "pending"), row
        capture.extend(send(master, b"\x1b"))
        capture.extend(send(master, b"2"))
        capture.extend(send(master, b"\x1b[D p"))
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 14, 40, 0, 0))
        os.kill(process.pid, signal.SIGWINCH)
        capture.extend(drain(master))
        capture.extend(send(master, b"\x1b"))
        capture.extend(send(master, b"1"))
        capture.extend(send(master, b" "))
        row = connection.execute("SELECT completed,reminder_state FROM tasks").fetchone()
        assert row == (1, "cancelled"), row
        capture.extend(send(master, b"q"))
        assert process.wait(timeout=3) == 0
        try:
            restored = termios.tcgetattr(slave)
            assert restored[3] & (termios.ECHO | termios.ICANON) == original[3] & (termios.ECHO | termios.ICANON)
        except termios.error as error:
            # macOS revokes the slave after its controlling session exits.
            if error.args[0] != 25:
                raise
        assert b"\x1b[?2004l" in capture, "bracketed paste was not disabled"
        assert b"\x1b[?1049l" in capture, "alternate screen was not restored"
    finally:
        if process.poll() is None:
            process.kill()
        os.close(master)
        os.close(slave)
        connection.close()
    backup = directory / "export.json"
    subprocess.run([str(binary), "--data-dir", str(directory), "export", str(backup)], check=True, capture_output=True)
    assert json.loads(backup.read_text())["tasks"][0]["title"] == "中文事项测试"
    preview = subprocess.run([str(binary), "--data-dir", str(directory), "import", str(backup)], check=True, capture_output=True, text=True)
    assert "未修改数据" in preview.stdout
    second, master, slave, original = start(directory)
    capture2 = drain(master)
    import re
    clean = lambda value: re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", value)
    assert "中文事项测试".encode() not in clean(capture2)  # completed items are hidden in the open view
    capture2 += send(master, b"\t")
    assert "中文事项测试".encode() in clean(capture2)
    send(master, b"q")
    assert second.wait(timeout=3) == 0
    os.close(master)
    os.close(slave)
    destination = root / "artifacts/pty-smoke.ansi"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(capture)
    print("PTY smoke passed: Unicode paste, save, menu/meadow switch, resize, completion, reopen, export preview, terminal restoration")

"""A real, isolated PTY session for development smoke checks on macOS/Linux."""
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import termios
import time

class Session:
    def __init__(self, binary, directory, screen, width=120, height=40):
        self.master, self.slave = pty.openpty()
        self.capture = bytearray()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
        def controlling_terminal():
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)
            os.tcsetpgrp(self.slave, os.getpgrp())
        self.process = subprocess.Popen([str(Path(binary).resolve()), "--data-dir", str(directory), "--color", "rgb", screen], stdin=self.slave, stdout=self.slave, stderr=self.slave, preexec_fn=controlling_terminal, env={**os.environ, "TERM": "xterm-256color", "COLORTERM": "truecolor"})
        deadline = time.monotonic() + 20
        while b"\x1b[?2004h" not in self.capture:
            self.drain(0.1)
            if self.process.poll() is not None or time.monotonic() > deadline:
                self.abort()
                raise RuntimeError("TUI failed to become ready")

    def drain(self, seconds=0.1):
        initial = len(self.capture)
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([self.master], [], [], 0.02)[0]:
                try:
                    self.capture.extend(os.read(self.master, 65536))
                except OSError:
                    break
        return len(self.capture) - initial

    def close(self):
        os.write(self.master, b"q")
        deadline = time.monotonic() + 3
        while self.process.poll() is None and time.monotonic() < deadline:
            self.drain(0.1)
        self.drain(0.1)
        code = self.process.poll()
        self.abort()
        if code != 0:
            raise RuntimeError(f"TUI did not exit cleanly: {code}")

    def abort(self):
        if self.process.poll() is None:
            self.process.kill()
        for handle in (self.master, self.slave):
            try:
                os.close(handle)
            except OSError:
                pass

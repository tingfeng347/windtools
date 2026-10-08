#!/usr/bin/env python3
"""Measure release TUI process CPU/RSS/output with 10k synthetic tasks."""
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import time
import uuid
from terminal_session import Session

root = Path(__file__).resolve().parents[1]
binary = root / "target/release/windtools"
records = []
with tempfile.TemporaryDirectory(prefix="windtools-perf-") as temporary:
    directory = Path(temporary)
    now = int(time.time())
    tasks = [{"id": str(uuid.uuid4()), "title": f"事项 {index} 中文测试", "note": "性能测试的合成数据", "completed": False, "remind_at": None, "reminder_state": "none", "last_error": None, "created_at": now, "updated_at": now, "revision": 0} for index in range(10000)]
    backup = directory / "synthetic.json"
    backup.write_text(json.dumps({"format_version": 1, "exported_at": now, "tasks": tasks}, ensure_ascii=False))
    subprocess.run([str(binary), "--data-dir", str(directory), "import", str(backup), "--yes"], check=True, capture_output=True)
    for screen, width, height in [("todo", 120, 40), ("meadow", 120, 40), ("meadow", 240, 80)]:
        session = Session(binary, directory, screen, width, height)
        try:
            session.drain(0.5)
            initial = len(session.capture)
            start = time.monotonic()
            session.drain(5)
            elapsed = time.monotonic() - start
            cpu, rss = subprocess.check_output(["ps", "-p", str(session.process.pid), "-o", "%cpu=,rss="], text=True).split()
            record = {"screen": screen, "size": f"{width}x{height}", "tasks": 10000, "cpu_percent_ps_sample": float(cpu), "rss_mib": round(int(rss)/1024, 2), "terminal_output_bytes_per_second": round((len(session.capture)-initial)/elapsed)}
            records.append(record)
        finally:
            session.close()
destination = root / "artifacts/pty-performance.json"
destination.parent.mkdir(parents=True, exist_ok=True)
destination.write_text(json.dumps(records, indent=2))
print(json.dumps(records, indent=2))

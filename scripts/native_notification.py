#!/usr/bin/env python3
"""Submit one clearly labelled native notification using a temporary todo list."""
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
with tempfile.TemporaryDirectory(prefix="windtools-notify-") as temporary:
    directory = Path(temporary)
    now = int(time.time())
    task = {"id": str(uuid.uuid4()), "title": "windtools 系统通知测试（临时数据）", "note": "此事项仅用于验证提醒提交，不写入个人清单。", "completed": False, "remind_at": now+12, "reminder_state": "pending", "last_error": None, "created_at": now, "updated_at": now, "revision": 0}
    backup = directory / "synthetic.json"
    backup.write_text(json.dumps({"format_version": 1, "exported_at": now, "tasks": [task]}, ensure_ascii=False))
    subprocess.run([str(binary), "--data-dir", str(directory), "import", str(backup), "--yes"], check=True, capture_output=True)
    session = Session(binary, directory, "meadow")
    try:
        connection = sqlite3.connect(directory / "windtools.sqlite3")
        deadline = time.monotonic()+30
        while True:
            session.drain(0.2)
            state, error = connection.execute("SELECT reminder_state,last_error FROM tasks").fetchone()
            if state in ("sent", "failed", "missed") or time.monotonic()>deadline:
                break
        connection.close()
        result = {"reminder_state": state, "error": error, "screen": "meadow", "actual_desktop_visibility_verified": False}
        destination = root / "artifacts/native-notification.json"
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(json.dumps(result, ensure_ascii=False, indent=2))
        print(json.dumps(result, ensure_ascii=False, indent=2))
        assert state == "sent", result
    finally:
        session.close()

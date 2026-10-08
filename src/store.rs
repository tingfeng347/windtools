use crate::model::{ReminderState, Task};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use directories::ProjectDirs;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

const COLUMNS: &str =
    "id,title,note,completed,remind_at,reminder_state,last_error,created_at,updated_at,revision";

pub struct Store {
    conn: Connection,
    pub directory: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backup {
    pub format_version: u32,
    pub exported_at: i64,
    pub tasks: Vec<Task>,
}

#[derive(Clone, Debug)]
pub struct Dispatch {
    pub task: Task,
    pub nonce: String,
}

pub fn data_directory(override_path: Option<PathBuf>) -> Result<PathBuf> {
    override_path
        .or_else(|| {
            ProjectDirs::from("", "", "windtools").map(|dirs| dirs.data_local_dir().to_owned())
        })
        .ok_or_else(|| anyhow::anyhow!("找不到用户数据目录，请使用 --data-dir 指定路径"))
}

impl Store {
    pub fn open(directory: &Path) -> Result<Self> {
        fs::create_dir_all(directory)
            .with_context(|| format!("无法创建数据目录 {}", directory.display()))?;
        let conn = Connection::open(directory.join("windtools.sqlite3"))?;
        Self::initialize(conn, directory.to_owned())
    }
    pub fn in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?, PathBuf::new())
    }
    fn initialize(conn: Connection, directory: PathBuf) -> Result<Self> {
        conn.busy_timeout(Duration::from_millis(75))?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 1 {
            bail!("数据格式来自较新版本，请升级 windtools；当前文件未修改");
        }
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.execute_batch("BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS tasks (
              id TEXT PRIMARY KEY, title TEXT NOT NULL, note TEXT NOT NULL, completed INTEGER NOT NULL,
              remind_at INTEGER, reminder_state TEXT NOT NULL, last_error TEXT,
              created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, revision INTEGER NOT NULL DEFAULT 0,
              dispatch_nonce TEXT);
            CREATE INDEX IF NOT EXISTS reminder_due ON tasks(reminder_state,remind_at);
            PRAGMA user_version=1; COMMIT;")?;
        Ok(Self { conn, directory })
    }
    pub fn list(&self) -> Result<Vec<Task>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM tasks ORDER BY completed ASC, CASE WHEN reminder_state='pending' THEN remind_at ELSE 9223372036854775807 END ASC, created_at DESC, id ASC"
        );
        let mut query = self.conn.prepare_cached(&sql)?;
        Ok(query
            .query_map([], task_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn get(&self, id: &str) -> Result<Option<Task>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM tasks WHERE id=?1"),
                [id],
                task_row,
            )
            .optional()?)
    }
    pub fn data_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .pragma_query_value(None, "data_version", |row| row.get(0))?)
    }
    pub fn insert(&self, task: &Task) -> Result<()> {
        task.validate()?;
        insert_row(&self.conn, task)
    }
    pub fn update(&self, task: &Task) -> Result<()> {
        task.validate()?;
        let changed = self.conn.execute("UPDATE tasks SET title=?2,note=?3,completed=?4,remind_at=?5,reminder_state=?6,last_error=?7,updated_at=?8,revision=revision+1,
            dispatch_nonce=CASE WHEN reminder_state=?6 AND remind_at IS ?5 AND completed=?4 THEN dispatch_nonce ELSE NULL END WHERE id=?1 AND revision=?9",
            params![task.id,task.title,task.note,task.completed,task.remind_at,task.reminder_state.as_str(),task.last_error,task.updated_at,task.revision])?;
        if changed == 0 {
            bail!("事项已被另一操作更新或删除，请重新打开；编辑内容仍保留");
        }
        Ok(())
    }
    pub fn delete(&self, task: &Task) -> Result<()> {
        if self.conn.execute(
            "DELETE FROM tasks WHERE id=?1 AND revision=?2",
            params![task.id, task.revision],
        )? == 0
        {
            bail!("事项已变化，请重新打开后再删除");
        }
        Ok(())
    }
    pub fn start_scheduler(&self, now: i64) -> Result<usize> {
        self.conn.execute("UPDATE tasks SET reminder_state='failed',last_error='上次发送被中断；如需再次提醒，请重新设置时间',dispatch_nonce=NULL,revision=revision+1 WHERE reminder_state='sending'", [])?;
        self.skip_missed(now)
    }
    pub fn skip_missed(&self, now: i64) -> Result<usize> {
        Ok(self.conn.execute("UPDATE tasks SET reminder_state='missed',dispatch_nonce=NULL,revision=revision+1 WHERE reminder_state='pending' AND completed=0 AND remind_at<=?1", [now])?)
    }
    pub fn claim_due(&mut self, now: i64, limit: usize) -> Result<Vec<Dispatch>> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let tasks = {
            let mut query = tx.prepare(&format!("SELECT {COLUMNS} FROM tasks WHERE completed=0 AND reminder_state='pending' AND remind_at<=?1 ORDER BY remind_at LIMIT ?2"))?;
            query
                .query_map(params![now, limit as i64], task_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut claimed = Vec::with_capacity(tasks.len());
        for mut task in tasks {
            let nonce = Uuid::new_v4().to_string();
            tx.execute("UPDATE tasks SET reminder_state='sending',dispatch_nonce=?2,revision=revision+1 WHERE id=?1 AND reminder_state='pending'", params![task.id,nonce])?;
            task.reminder_state = ReminderState::Sending;
            task.revision += 1;
            claimed.push(Dispatch { task, nonce });
        }
        tx.commit()?;
        Ok(claimed)
    }
    pub fn dispatch_is_current(&self, id: &str, nonce: &str) -> Result<bool> {
        Ok(self.conn.query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND dispatch_nonce=?2 AND reminder_state='sending' AND completed=0)", params![id,nonce], |row| row.get(0))?)
    }
    pub fn finish_dispatch(&self, id: &str, nonce: &str, outcome: &Result<()>) -> Result<bool> {
        let error = outcome.as_ref().err().map(|value| format!("{value:#}"));
        let state = if error.is_some() { "failed" } else { "sent" };
        Ok(self.conn.execute("UPDATE tasks SET reminder_state=?3,last_error=?4,dispatch_nonce=NULL,revision=revision+1 WHERE id=?1 AND dispatch_nonce=?2 AND reminder_state='sending'", params![id,nonce,state,error])? > 0)
    }
    pub fn backup(&self) -> Result<Backup> {
        Ok(Backup {
            format_version: 1,
            exported_at: Utc::now().timestamp(),
            tasks: self.list()?,
        })
    }
    pub fn default_backup_path(&self) -> PathBuf {
        self.directory.join("backups").join(format!(
            "windtools-{}-{}.json",
            Utc::now().format("%Y%m%d-%H%M%S"),
            &Uuid::new_v4().to_string()[..8]
        ))
    }
    pub fn export(&self, destination: &Path) -> Result<()> {
        write_backup(&self.backup()?, destination)
    }
    pub fn restore(&mut self, backup: &Backup, now: i64) -> Result<PathBuf> {
        validate_backup(backup)?;
        let safety_path = self.default_backup_path();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let before = {
            let mut stmt = tx.prepare(&format!("SELECT {COLUMNS} FROM tasks"))?;
            let tasks = stmt
                .query_map([], task_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Backup {
                format_version: 1,
                exported_at: now,
                tasks,
            }
        };
        write_backup(&before, &safety_path).context("恢复前的备份失败；原数据未修改")?;
        tx.execute("DELETE FROM tasks", [])?;
        for original in &backup.tasks {
            let mut task = original.clone();
            task.revision = 0;
            if task.reminder_state == ReminderState::Sending {
                task.reminder_state = ReminderState::Failed;
                task.last_error = Some("备份中的发送未完成；请重新设置提醒时间".into());
            } else if task.reminder_state == ReminderState::Pending
                && task.remind_at.is_some_and(|at| at <= now)
            {
                task.reminder_state = ReminderState::Missed;
            }
            insert_row(&tx, &task)?;
        }
        tx.commit()?;
        Ok(safety_path)
    }
}

fn task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let raw: String = row.get(5)?;
    let reminder_state = ReminderState::parse(&raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, error.into())
    })?;
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        note: row.get(2)?,
        completed: row.get(3)?,
        remind_at: row.get(4)?,
        reminder_state,
        last_error: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        revision: row.get(9)?,
    })
}
fn insert_row(conn: &Connection, task: &Task) -> Result<()> {
    conn.execute("INSERT INTO tasks(id,title,note,completed,remind_at,reminder_state,last_error,created_at,updated_at,revision) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![task.id,task.title,task.note,task.completed,task.remind_at,task.reminder_state.as_str(),task.last_error,task.created_at,task.updated_at,task.revision])?;
    Ok(())
}
pub fn read_backup(path: &Path) -> Result<Backup> {
    let file = fs::File::open(path).with_context(|| format!("无法打开 {}", path.display()))?;
    if file.metadata()?.len() > 16 * 1024 * 1024 {
        bail!("备份超过 16 MiB，请检查文件");
    }
    let backup: Backup = serde_json::from_reader(file).context("备份不是有效的 windtools JSON")?;
    validate_backup(&backup)?;
    Ok(backup)
}
pub fn validate_backup(backup: &Backup) -> Result<()> {
    if backup.format_version != 1 {
        bail!("不支持此备份版本");
    }
    if backup.tasks.len() > 100_000 {
        bail!("备份事项过多");
    }
    let mut ids = HashSet::new();
    for task in &backup.tasks {
        task.validate()?;
        if !ids.insert(&task.id) {
            bail!("备份包含重复事项 ID");
        }
    }
    Ok(())
}
pub fn write_backup(backup: &Backup, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temp, backup)?;
    temp.write_all(b"\n")?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(destination)
        .map_err(|error| error.error)
        .with_context(|| format!("无法保存备份 {}；已有文件不会被覆盖", destination.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durable_edits_and_stale_writes_are_detected() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let task = Task::new("提交中文材料", "附件", Some(200), 100).unwrap();
        store.insert(&task).unwrap();
        let mut edited = task.clone();
        edited.title = "新标题".into();
        store.update(&edited).unwrap();
        assert!(store.update(&task).is_err());
        drop(store);
        assert_eq!(
            Store::open(dir.path())
                .unwrap()
                .get(&task.id)
                .unwrap()
                .unwrap()
                .title,
            "新标题"
        );
    }
    #[test]
    fn two_instances_claim_each_reminder_once_and_cancel_invalidates_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = Store::open(dir.path()).unwrap();
        let mut second = Store::open(dir.path()).unwrap();
        let task = Task::new("提醒", "", Some(200), 100).unwrap();
        first.insert(&task).unwrap();
        let claimed = first.claim_due(200, 10).unwrap();
        assert_eq!(claimed.len(), 1);
        assert!(second.claim_due(200, 10).unwrap().is_empty());
        let mut task = second.get(&task.id).unwrap().unwrap();
        task.completed = true;
        task.reminder_state = ReminderState::Cancelled;
        second.update(&task).unwrap();
        assert!(
            !first
                .dispatch_is_current(&task.id, &claimed[0].nonce)
                .unwrap()
        );
        assert!(
            !first
                .finish_dispatch(&task.id, &claimed[0].nonce, &Ok(()))
                .unwrap()
        );
    }
    #[test]
    fn missed_and_interrupted_reminders_never_replay() {
        let mut store = Store::in_memory().unwrap();
        let missed = Task::new("旧事项", "", Some(150), 100).unwrap();
        let interrupted = Task::new("中断", "", Some(160), 100).unwrap();
        store.insert(&interrupted).unwrap();
        store.claim_due(160, 10).unwrap();
        store.insert(&missed).unwrap();
        store.start_scheduler(180).unwrap();
        assert_eq!(
            store.get(&missed.id).unwrap().unwrap().reminder_state,
            ReminderState::Missed
        );
        assert_eq!(
            store.get(&interrupted.id).unwrap().unwrap().reminder_state,
            ReminderState::Failed
        );
        assert!(store.claim_due(300, 10).unwrap().is_empty());
    }
    #[test]
    fn restore_is_validated_preserves_safety_backup_and_skips_old_reminders() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let original = Task::new("原数据", "", None, 100).unwrap();
        store.insert(&original).unwrap();
        let incoming = Task::new("恢复事项", "", Some(200), 100).unwrap();
        let mut backup = Backup {
            format_version: 1,
            exported_at: 100,
            tasks: vec![incoming.clone(), incoming.clone()],
        };
        assert!(store.restore(&backup, 300).is_err());
        assert!(store.get(&original.id).unwrap().is_some());
        backup.tasks.pop();
        let safety = store.restore(&backup, 300).unwrap();
        assert_eq!(read_backup(&safety).unwrap().tasks[0].id, original.id);
        assert_eq!(
            store.list().unwrap()[0].reminder_state,
            ReminderState::Missed
        );
        assert!(!store.list().unwrap()[0].completed);
        assert!(write_backup(&backup, &safety).is_err());
    }
}

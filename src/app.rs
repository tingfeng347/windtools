use crate::{
    input::TextInput,
    meadow::Meadow,
    model::{
        ReminderState, Task, ambiguous_reminder_candidates, local_time, parse_reminder,
        validate_text,
    },
    notifier::{DesktopNotifier, NotificationWorker},
    store::{Backup, Store, read_backup},
};
use anyhow::{Result, bail};
use chrono::Utc;
use fs2::FileExt;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{
    fs::{File, OpenOptions},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Menu,
    Todo,
    Meadow,
    Editor,
    Details,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Open,
    All,
    Done,
}
impl Filter {
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "未完成",
            Self::All => "全部",
            Self::Done => "已完成",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Open => Self::All,
            Self::All => Self::Done,
            Self::Done => Self::Open,
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub ascii: bool,
    pub mono: bool,
    pub ansi: bool,
    pub seed: u64,
    pub demo: bool,
}

pub struct Editor {
    pub original: Option<Task>,
    pub fields: [TextInput; 3],
    pub focus: usize,
    pub original_time: String,
}
impl Editor {
    fn new(task: Option<Task>) -> Self {
        let original_time = task
            .as_ref()
            .and_then(|task| task.remind_at)
            .map(|at| local_time(at, "%Y-%m-%dT%H:%M:%S%:z"))
            .unwrap_or_default();
        Self {
            fields: [
                TextInput::new(
                    task.as_ref()
                        .map(|task| task.title.clone())
                        .unwrap_or_default(),
                ),
                TextInput::new(
                    task.as_ref()
                        .map(|task| task.note.clone())
                        .unwrap_or_default(),
                ),
                TextInput::new(original_time.clone()),
            ],
            original: task,
            focus: 0,
            original_time,
        }
    }
    pub fn changed(&self) -> bool {
        self.fields[0].text
            != self
                .original
                .as_ref()
                .map(|task| task.title.as_str())
                .unwrap_or("")
            || self.fields[1].text
                != self
                    .original
                    .as_ref()
                    .map(|task| task.note.as_str())
                    .unwrap_or("")
            || self.fields[2].text != self.original_time
    }
}
#[derive(Clone)]
pub enum Overlay {
    Help,
    Error(String),
    TimeChoice([i64; 2]),
    Delete(Task),
    Discard { exit: bool },
    Export(TextInput),
    Import(TextInput),
    Restore { backup: Backup, source: String },
}

pub struct App {
    pub store: Store,
    pub tasks: Vec<Task>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub scroll: usize,
    pub screen: Screen,
    pub menu_index: usize,
    pub filter: Filter,
    pub search: TextInput,
    pub searching: bool,
    pub editor: Option<Editor>,
    pub overlay: Option<Overlay>,
    pub return_overlay: Option<Overlay>,
    pub message: String,
    pub message_error: bool,
    pub options: Options,
    pub meadow: Meadow,
    pub dirty: bool,
    pub quit: bool,
    pub scheduler_active: bool,
    pub detail_scroll: u16,
    pub overlay_scroll: u16,
    worker: Option<NotificationWorker>,
    leader_lock: Option<File>,
    last_scheduler: i64,
    last_poll: Instant,
    data_version: i64,
}

impl App {
    pub fn new(store: Store, screen: Screen, options: Options) -> Result<Self> {
        let now = Utc::now().timestamp();
        let mut app = Self {
            store,
            tasks: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            scroll: 0,
            screen,
            menu_index: 0,
            filter: Filter::Open,
            search: TextInput::default(),
            searching: false,
            editor: None,
            overlay: None,
            return_overlay: None,
            message: if options.demo {
                "演示数据，不发送系统通知".into()
            } else {
                "数据已保存".into()
            },
            message_error: false,
            options,
            meadow: Meadow::new(options.seed),
            dirty: true,
            quit: false,
            scheduler_active: false,
            detail_scroll: 0,
            overlay_scroll: 0,
            worker: None,
            leader_lock: None,
            last_scheduler: now,
            last_poll: Instant::now(),
            data_version: 0,
        };
        if !options.demo {
            app.acquire_scheduler(now)?;
            app.worker = Some(NotificationWorker::new(
                app.store.directory.clone(),
                DesktopNotifier,
            ));
        }
        app.reload()?;
        Ok(app)
    }
    pub fn demo(screen: Screen, options: Options) -> Result<Self> {
        let now = Utc::now().timestamp();
        let store = Store::in_memory()?;
        store.insert(&Task::new(
            "提交材料",
            "检查附件与收件人，然后确认提交结果。",
            Some(now + 3600),
            now,
        )?)?;
        store.insert(&Task::new(
            "整理会议笔记",
            "把讨论内容整理为行动项。",
            None,
            now - 10,
        )?)?;
        let mut missed = Task::new("预约体检", "选一个方便的上午。", Some(now + 3600), now - 20)?;
        missed.remind_at = Some(now - 3600);
        missed.reminder_state = ReminderState::Missed;
        store.insert(&missed)?;
        let mut done = Task::new("更新项目说明", "", None, now - 30)?;
        done.completed = true;
        store.insert(&done)?;
        Self::new(
            store,
            screen,
            Options {
                demo: true,
                ..options
            },
        )
    }
    fn acquire_scheduler(&mut self, now: i64) -> Result<()> {
        if self.leader_lock.is_some() || self.options.demo {
            return Ok(());
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.store.directory.join("reminders.lock"))?;
        match FileExt::try_lock_exclusive(&lock) {
            Ok(()) => {
                self.store.start_scheduler(now)?;
                self.leader_lock = Some(lock);
                self.scheduler_active = true;
                self.last_scheduler = now;
                self.dirty = true;
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }
    pub fn reload(&mut self) -> Result<()> {
        let id = self.current().map(|task| task.id.clone());
        self.tasks = self.store.list()?;
        self.data_version = self.store.data_version()?;
        self.refilter();
        if let Some(id) = id
            && let Some(index) = self
                .filtered
                .iter()
                .position(|&index| self.tasks[index].id == id)
        {
            self.selected = index;
        }
        self.dirty = true;
        Ok(())
    }
    pub fn refilter(&mut self) {
        let query = self.search.text.to_lowercase();
        self.filtered = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                (self.filter == Filter::All || task.completed == (self.filter == Filter::Done))
                    && (query.is_empty() || task.title.to_lowercase().contains(&query))
            })
            .map(|(index, _)| index)
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
        self.scroll = self.scroll.min(self.selected);
        self.dirty = true;
    }
    pub fn current(&self) -> Option<&Task> {
        self.filtered
            .get(self.selected)
            .map(|&index| &self.tasks[index])
    }
    pub fn status(&mut self, message: impl Into<String>, error: bool) {
        self.message = message.into();
        self.message_error = error;
        self.dirty = true;
    }
    pub fn tick(&mut self) -> Result<()> {
        if self.last_poll.elapsed() < Duration::from_millis(250) {
            return Ok(());
        }
        self.last_poll = Instant::now();
        let now = Utc::now().timestamp();
        let mut changed = false;
        if !self.options.demo {
            self.acquire_scheduler(now)?;
            if let Some(worker) = &self.worker {
                while let Ok(result) = worker.results.try_recv() {
                    if self
                        .store
                        .finish_dispatch(&result.id, &result.nonce, &result.result)?
                    {
                        changed = true;
                        if let Err(error) = result.result {
                            self.message = format!("通知发送失败：{error:#}");
                            self.message_error = true;
                        }
                    }
                }
            }
            if self.scheduler_active {
                if now - self.last_scheduler > 5 {
                    changed |= self.store.skip_missed(now)? > 0;
                }
                self.last_scheduler = now;
                let jobs = self.store.claim_due(now, 8)?;
                changed |= !jobs.is_empty();
                if let Some(worker) = &self.worker {
                    for job in jobs {
                        let id = job.task.id.clone();
                        let nonce = job.nonce.clone();
                        if let Err(error) = worker.submit(job) {
                            self.store.finish_dispatch(&id, &nonce, &Err(error))?;
                        }
                    }
                }
            }
        }
        if changed || self.store.data_version()? != self.data_version {
            self.reload()?;
        }
        Ok(())
    }
    pub fn key(&mut self, key: KeyEvent) {
        self.dirty = true;
        if let Err(error) = self.handle_key(key) {
            self.status(format!("{error:#}"), true);
        }
    }
    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char('c' | 'q')) {
            self.request_quit();
            return Ok(());
        }
        if key.code == KeyCode::F(2)
            && self.message_error
            && !matches!(self.overlay, Some(Overlay::Error(_)))
        {
            self.return_overlay = self.overlay.take();
            self.overlay_scroll = 0;
            self.overlay = Some(Overlay::Error(self.message.clone()));
            return Ok(());
        }
        if self.overlay.is_some() {
            return self.overlay_key(key);
        }
        if self.screen == Screen::Editor {
            if key.code == KeyCode::F(5) {
                if let Some(candidates) = self
                    .editor
                    .as_ref()
                    .and_then(|editor| ambiguous_reminder_candidates(&editor.fields[2].text))
                {
                    self.overlay = Some(Overlay::TimeChoice(candidates));
                    self.overlay_scroll = 0;
                }
                return Ok(());
            }
            if key.code == KeyCode::Esc {
                if self.editor.as_ref().is_some_and(Editor::changed) {
                    self.overlay_scroll = 0;
                    self.overlay = Some(Overlay::Discard { exit: false });
                } else {
                    self.editor = None;
                    self.screen = Screen::Todo;
                }
            } else if ctrl && matches!(key.code, KeyCode::Char('s') | KeyCode::Enter) {
                self.save_editor()?;
            } else if let Some(editor) = &mut self.editor {
                match key.code {
                    KeyCode::Tab | KeyCode::Enter => editor.focus = (editor.focus + 1) % 3,
                    KeyCode::BackTab => editor.focus = (editor.focus + 2) % 3,
                    _ => editor.fields[editor.focus].key(key),
                }
            }
            return Ok(());
        }
        if self.searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => self.searching = false,
                _ => self.search.key(key),
            }
            self.refilter();
            return Ok(());
        }
        if key.code == KeyCode::Char('?') {
            self.overlay_scroll = 0;
            self.overlay = Some(Overlay::Help);
            return Ok(());
        }
        if key.code == KeyCode::Char('q') {
            self.request_quit();
            return Ok(());
        }
        match self.screen {
            Screen::Menu => match key.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Char('j' | 'k') | KeyCode::Tab => {
                    self.menu_index = 1 - self.menu_index
                }
                KeyCode::Char('1') => self.screen = Screen::Todo,
                KeyCode::Char('2') => self.screen = Screen::Meadow,
                KeyCode::Enter => {
                    self.screen = if self.menu_index == 0 {
                        Screen::Todo
                    } else {
                        Screen::Meadow
                    }
                }
                _ => {}
            },
            Screen::Todo => match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected = (self.selected + 1).min(self.filtered.len().saturating_sub(1))
                }
                KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
                KeyCode::Home => self.selected = 0,
                KeyCode::End => self.selected = self.filtered.len().saturating_sub(1),
                KeyCode::PageDown => {
                    self.selected = (self.selected + 10).min(self.filtered.len().saturating_sub(1))
                }
                KeyCode::PageUp => self.selected = self.selected.saturating_sub(10),
                KeyCode::Esc => self.screen = Screen::Menu,
                KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
                    self.filter = self.filter.next();
                    self.selected = 0;
                    self.scroll = 0;
                    self.refilter();
                }
                KeyCode::Char('/') => self.searching = true,
                KeyCode::Char('a') => {
                    self.editor = Some(Editor::new(None));
                    self.screen = Screen::Editor;
                    self.status("填写事项；Ctrl+S 保存，Esc 返回", false);
                }
                KeyCode::Char('e') => {
                    if let Some(task) = self.current().cloned() {
                        self.editor = Some(Editor::new(Some(task)));
                        self.screen = Screen::Editor;
                    }
                }
                KeyCode::Char(' ') => self.toggle_current()?,
                KeyCode::Char('d') => {
                    if let Some(task) = self.current().cloned() {
                        self.overlay_scroll = 0;
                        self.overlay = Some(Overlay::Delete(task));
                    }
                }
                KeyCode::Enter => {
                    if self.current().is_some() {
                        self.detail_scroll = 0;
                        self.screen = Screen::Details;
                    }
                }
                KeyCode::Char('b') => {
                    self.overlay = Some(Overlay::Export(TextInput::new(
                        self.store
                            .default_backup_path()
                            .to_string_lossy()
                            .to_string(),
                    )))
                }
                KeyCode::Char('i') => self.overlay = Some(Overlay::Import(TextInput::default())),
                _ => {}
            },
            Screen::Details => match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.detail_scroll = self.detail_scroll.saturating_add(1)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1)
                }
                KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(10),
                KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(10),
                KeyCode::Home => self.detail_scroll = 0,
                KeyCode::End => self.detail_scroll = u16::MAX,
                KeyCode::Esc | KeyCode::Enter => self.screen = Screen::Todo,
                KeyCode::Char('e') => {
                    if let Some(task) = self.current().cloned() {
                        self.editor = Some(Editor::new(Some(task)));
                        self.screen = Screen::Editor;
                    }
                }
                _ => {}
            },
            Screen::Meadow => match key.code {
                KeyCode::Esc => self.screen = Screen::Menu,
                KeyCode::Left => self.meadow.direction = -1.0,
                KeyCode::Right => self.meadow.direction = 1.0,
                KeyCode::Char(' ') => self.meadow.gust(),
                KeyCode::Char('p') => self.meadow.paused = !self.meadow.paused,
                _ => {}
            },
            Screen::Editor => {}
        }
        Ok(())
    }
    fn request_quit(&mut self) {
        if self.editor.as_ref().is_some_and(Editor::changed) {
            self.overlay_scroll = 0;
            self.overlay = Some(Overlay::Discard { exit: true });
        } else {
            self.quit = true;
        }
    }
    pub fn paste(&mut self, text: &str) {
        if let Some(Overlay::Export(input) | Overlay::Import(input)) = &mut self.overlay {
            input.insert(text, false);
        } else if self.overlay.is_none() {
            if let Some(editor) = &mut self.editor {
                editor.fields[editor.focus].insert(text, editor.focus == 1);
            } else if self.searching {
                self.search.insert(text, false);
                self.refilter();
            }
        }
        self.dirty = true;
    }
    fn toggle_current(&mut self) -> Result<()> {
        if let Some(mut task) = self.current().cloned() {
            task.completed = !task.completed;
            task.updated_at = Utc::now().timestamp();
            if task.completed
                && matches!(
                    task.reminder_state,
                    ReminderState::Pending | ReminderState::Sending
                )
            {
                task.reminder_state = ReminderState::Cancelled;
                task.last_error = None;
            }
            self.store.update(&task)?;
            self.reload()?;
            self.status("数据已保存", false);
        }
        Ok(())
    }
    fn save_editor(&mut self) -> Result<()> {
        let Some(editor) = &self.editor else {
            return Ok(());
        };
        let now = Utc::now().timestamp();
        validate_text(&editor.fields[0].text, &editor.fields[1].text)?;
        let original = editor.original.as_ref();
        let remind_at = if original.is_some() && editor.fields[2].text == editor.original_time {
            original.and_then(|task| task.remind_at)
        } else {
            parse_reminder(&editor.fields[2].text, now)?
        };
        if let Some(original) = original {
            let mut task = self
                .store
                .get(&original.id)?
                .ok_or_else(|| anyhow::anyhow!("事项已删除；编辑内容仍保留"))?;
            if task.title != original.title
                || task.note != original.note
                || task.remind_at != original.remind_at
                || task.completed != original.completed
            {
                bail!("事项已在另一窗口更新，请取消后重新编辑；当前输入仍保留");
            }
            task.title = editor.fields[0].text.trim().into();
            task.note = editor.fields[1].text.clone();
            if task.remind_at != remind_at {
                task.remind_at = remind_at;
                task.last_error = None;
                task.reminder_state = if remind_at.is_none() {
                    ReminderState::None
                } else if task.completed {
                    ReminderState::Cancelled
                } else {
                    ReminderState::Pending
                };
            }
            task.updated_at = now;
            self.store.update(&task)?;
        } else {
            self.store.insert(&Task::new(
                &editor.fields[0].text,
                &editor.fields[1].text,
                remind_at,
                now,
            )?)?;
        }
        self.editor = None;
        self.screen = Screen::Todo;
        self.reload()?;
        self.status("数据已保存", false);
        Ok(())
    }
    fn overlay_key(&mut self, key: KeyEvent) -> Result<()> {
        if matches!(
            self.overlay,
            Some(
                Overlay::Help
                    | Overlay::Error(_)
                    | Overlay::Restore { .. }
                    | Overlay::Delete(_)
                    | Overlay::Discard { .. }
                    | Overlay::TimeChoice(_)
            )
        ) {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.overlay_scroll = self.overlay_scroll.saturating_add(1);
                    return Ok(());
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.overlay_scroll = self.overlay_scroll.saturating_sub(1);
                    return Ok(());
                }
                KeyCode::PageDown => {
                    self.overlay_scroll = self.overlay_scroll.saturating_add(5);
                    return Ok(());
                }
                KeyCode::PageUp => {
                    self.overlay_scroll = self.overlay_scroll.saturating_sub(5);
                    return Ok(());
                }
                KeyCode::Home => {
                    self.overlay_scroll = 0;
                    return Ok(());
                }
                KeyCode::End => {
                    self.overlay_scroll = u16::MAX;
                    return Ok(());
                }
                _ => {}
            }
        }
        let original = self.overlay.take().unwrap();
        let mut next = Some(original.clone());
        let result = (|| -> Result<()> {
            match original {
                Overlay::Help => {
                    if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?')) {
                        next = None;
                    }
                }
                Overlay::Error(_) => {
                    if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                        next = self.return_overlay.take();
                        self.overlay_scroll = 0;
                    }
                }
                Overlay::TimeChoice(candidates) => {
                    let index = match key.code {
                        KeyCode::Char('1') => Some(0),
                        KeyCode::Char('2') => Some(1),
                        _ => None,
                    };
                    if let Some(index) = index {
                        if let Some(editor) = &mut self.editor {
                            editor.fields[2] = TextInput::new(local_time(
                                candidates[index],
                                "%Y-%m-%dT%H:%M:%S%:z",
                            ));
                        }
                        self.message_error = false;
                        next = None;
                    } else if key.code == KeyCode::Esc {
                        next = None;
                    }
                }
                Overlay::Delete(task) => {
                    if key.code == KeyCode::Char('y') {
                        self.store.delete(&task)?;
                        self.reload()?;
                        self.status("事项已删除，数据已保存", false);
                        next = None;
                    } else if matches!(key.code, KeyCode::Esc | KeyCode::Char('n')) {
                        next = None;
                    }
                }
                Overlay::Discard { exit } => {
                    if key.code == KeyCode::Char('y') {
                        self.editor = None;
                        if exit {
                            self.quit = true;
                        } else {
                            self.screen = Screen::Todo;
                        }
                        self.status("已放弃未保存的修改", false);
                        next = None;
                    } else if matches!(key.code, KeyCode::Esc | KeyCode::Char('n')) {
                        next = None;
                    }
                }
                Overlay::Export(mut input) => {
                    if key.code == KeyCode::Esc {
                        next = None;
                    } else if key.code == KeyCode::Enter {
                        let path = std::path::PathBuf::from(input.text.trim());
                        if path.as_os_str().is_empty() {
                            bail!("请输入备份路径");
                        }
                        self.store.export(&path)?;
                        self.status(format!("备份已保存：{}", path.display()), false);
                        next = None;
                    } else {
                        input.key(key);
                        next = Some(Overlay::Export(input));
                    }
                }
                Overlay::Import(mut input) => {
                    if key.code == KeyCode::Esc {
                        next = None;
                    } else if key.code == KeyCode::Enter {
                        let backup = read_backup(std::path::Path::new(input.text.trim()))?;
                        next = Some(Overlay::Restore {
                            backup,
                            source: input.text,
                        });
                        self.overlay_scroll = 0;
                    } else {
                        input.key(key);
                        next = Some(Overlay::Import(input));
                    }
                }
                Overlay::Restore { backup, .. } => {
                    if key.code == KeyCode::Char('y') {
                        let safety = self.store.restore(&backup, Utc::now().timestamp())?;
                        self.reload()?;
                        self.status(
                            format!(
                                "已恢复 {} 项；原数据备份：{}",
                                backup.tasks.len(),
                                safety.display()
                            ),
                            false,
                        );
                        next = None;
                    } else if matches!(key.code, KeyCode::Esc | KeyCode::Char('n')) {
                        next = None;
                    }
                }
            }
            Ok(())
        })();
        self.overlay = next;
        result
    }
}
impl Drop for App {
    fn drop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    #[test]
    fn sqlite_write_failure_keeps_unsaved_form() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(
            Store::open(dir.path()).unwrap(),
            Screen::Todo,
            Options {
                demo: true,
                ..Options::default()
            },
        )
        .unwrap();
        app.key(key(KeyCode::Char('a')));
        app.paste("保存失败也要保留");
        let locked = rusqlite::Connection::open(dir.path().join("windtools.sqlite3")).unwrap();
        locked.execute_batch("BEGIN IMMEDIATE;").unwrap();
        app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(app.message_error);
        assert_eq!(app.screen, Screen::Editor);
        assert_eq!(
            app.editor.as_ref().unwrap().fields[0].text,
            "保存失败也要保留"
        );
        locked.execute_batch("ROLLBACK;").unwrap();
        app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(!app.message_error);
        assert_eq!(app.store.list().unwrap().len(), 1);
    }
    struct FailingNotifier(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl crate::notifier::Notifier for FailingNotifier {
        fn send(&self, _: &str, _: &str) -> Result<()> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            bail!("test notification service unavailable")
        }
    }
    #[test]
    fn reminder_worker_runs_across_screens_and_records_failures_once() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(
            Store::open(dir.path()).unwrap(),
            Screen::Menu,
            Options::default(),
        )
        .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        app.worker = Some(NotificationWorker::new(
            dir.path().to_owned(),
            FailingNotifier(Arc::clone(&calls)),
        ));
        let now = Utc::now().timestamp();
        let mut task = Task::new("跨页面提醒", "", Some(now + 60), now).unwrap();
        task.remind_at = Some(now);
        app.store.insert(&task).unwrap();
        app.screen = Screen::Meadow;
        let start = Instant::now();
        loop {
            app.last_poll = Instant::now() - Duration::from_secs(1);
            app.tick().unwrap();
            if app.store.get(&task.id).unwrap().unwrap().reminder_state == ReminderState::Failed {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(app.message_error);
        app.screen = Screen::Menu;
        app.last_poll = Instant::now() - Duration::from_secs(1);
        app.tick().unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let mut missed = Task::new("暂停期间", "", Some(now + 60), now).unwrap();
        missed.remind_at = Some(now);
        app.store.insert(&missed).unwrap();
        app.last_scheduler = now - 10;
        app.last_poll = Instant::now() - Duration::from_secs(1);
        app.tick().unwrap();
        assert_eq!(
            app.store.get(&missed.id).unwrap().unwrap().reminder_state,
            ReminderState::Missed
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn form_validation_preserves_input_and_completion_cancels_reminder() {
        let mut app = App::demo(Screen::Todo, Options::default()).unwrap();
        app.key(key(KeyCode::Char('a')));
        app.paste("中文新事项");
        app.editor.as_mut().unwrap().fields[2] = TextInput::new("bad time");
        app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(app.screen, Screen::Editor);
        assert!(app.message_error);
        assert_eq!(app.editor.as_ref().unwrap().fields[0].text, "中文新事项");
        app.editor.as_mut().unwrap().fields[2] = TextInput::new("+10m");
        app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(app.screen, Screen::Todo);
        let index = app
            .filtered
            .iter()
            .position(|&index| app.tasks[index].title == "中文新事项")
            .unwrap();
        app.selected = index;
        let id = app.current().unwrap().id.clone();
        app.key(key(KeyCode::Char(' ')));
        let task = app.store.get(&id).unwrap().unwrap();
        assert!(task.completed);
        assert_eq!(task.reminder_state, ReminderState::Cancelled);
    }
    #[test]
    fn unified_menu_keeps_session_and_explicitly_discards_unsaved_input() {
        let mut app = App::demo(Screen::Todo, Options::default()).unwrap();
        app.key(key(KeyCode::Esc));
        app.key(key(KeyCode::Char('2')));
        assert_eq!(app.screen, Screen::Meadow);
        app.key(key(KeyCode::Esc));
        app.key(key(KeyCode::Char('1')));
        assert_eq!(app.screen, Screen::Todo);
        app.key(key(KeyCode::Char('a')));
        app.paste("未保存");
        app.key(key(KeyCode::Esc));
        assert!(matches!(app.overlay, Some(Overlay::Discard { .. })));
        app.key(key(KeyCode::Char('n')));
        assert_eq!(app.screen, Screen::Editor);
        assert!(app.editor.is_some());
    }
}

use crate::{
    model::local_time,
    store::{Dispatch, Store},
};
use anyhow::{Result, anyhow};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
};

pub trait Notifier: Send + 'static {
    fn send(&self, title: &str, body: &str) -> Result<()>;
}
pub struct DesktopNotifier;
impl Notifier for DesktopNotifier {
    #[cfg(not(target_os = "macos"))]
    fn send(&self, title: &str, body: &str) -> Result<()> {
        notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .appname("windtools")
            .show()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn send(&self, title: &str, body: &str) -> Result<()> {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        // User text is passed as arguments, never interpolated into AppleScript.
        let script = "on run argv\ndisplay notification (item 2 of argv) with title (item 1 of argv)\nend run";
        let mut child = Command::new("/usr/bin/osascript")
            .args(["-e", script, "--", title, body])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let start = Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                let output = child.wait_with_output()?;
                if !output.status.success() {
                    return Err(anyhow!(
                        "系统通知调用失败：{}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
                return Ok(());
            }
            if start.elapsed() > Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow!("系统通知调用超时；请检查通知设置"));
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}
pub struct NotificationResult {
    pub id: String,
    pub nonce: String,
    pub result: Result<()>,
}
pub struct NotificationWorker {
    sender: SyncSender<Dispatch>,
    pub results: Receiver<NotificationResult>,
    stopped: Arc<AtomicBool>,
}
impl NotificationWorker {
    pub fn new(directory: PathBuf, notifier: impl Notifier) -> Self {
        let (sender, jobs) = mpsc::sync_channel::<Dispatch>(32);
        let (results_sender, results) = mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopped);
        thread::spawn(move || {
            let store = Store::open(&directory);
            while let Ok(job) = jobs.recv() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let result = match &store {
                    Ok(store) => match store.dispatch_is_current(&job.task.id, &job.nonce) {
                        Ok(true) => notifier.send(
                            "windtools · 待办提醒",
                            &format!(
                                "{}\n提醒时间：{}",
                                job.task.title,
                                local_time(job.task.remind_at.unwrap_or_default(), "%m-%d %H:%M")
                            ),
                        ),
                        Ok(false) => continue,
                        Err(error) => Err(error),
                    },
                    Err(error) => Err(anyhow!("通知读取数据失败：{error:#}")),
                };
                if results_sender
                    .send(NotificationResult {
                        id: job.task.id,
                        nonce: job.nonce,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            sender,
            results,
            stopped,
        }
    }
    pub fn submit(&self, job: Dispatch) -> Result<()> {
        self.sender
            .try_send(job)
            .map_err(|_| anyhow!("通知队列已满或不可用，请重新设置提醒"))
    }
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}
impl Drop for NotificationWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

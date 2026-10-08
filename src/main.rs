use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::{
        cursor::Show,
        event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
        execute,
    },
    style::Color,
};
use serde::Serialize;
use std::{
    fs,
    io::{self, IsTerminal},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windtools::{
    app::{App, Options, Screen},
    store::{Store, data_directory, read_backup},
    ui,
};

#[derive(Parser)]
#[command(
    version,
    about = "本地待办与字符原野 / Local todos and a living terminal meadow"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(long, global = true, help = "自定义本地数据目录")]
    data_dir: Option<PathBuf>,
    #[arg(long, global = true, help = "使用 ASCII 景观字符和英文操作标签")]
    ascii: bool,
    #[arg(long, global = true, value_enum, default_value = "auto")]
    color: ColorMode,
    #[arg(long,global=true,default_value_t=30,value_parser=clap::value_parser!(u16).range(1..=60))]
    fps: u16,
    #[arg(long, global = true, default_value_t = 42)]
    seed: u64,
    #[arg(long, global = true, help = "临时演示清单，不读写真实清单、不发送通知")]
    demo: bool,
}
#[derive(Subcommand)]
enum Command {
    /// 打开本地待办清单
    Todo,
    /// 进入风过原野
    Meadow,
    /// 导出备份（不会覆盖已有文件）
    Export { path: PathBuf },
    /// 校验备份并预览；--yes 确认恢复，恢复前自动备份
    Import {
        path: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    /// 显示本地数据目录
    DataPath,
    #[command(hide = true)]
    Snapshot {
        #[arg(value_enum)]
        screen: SnapshotScreen,
        #[arg(long, default_value_t = 120)]
        width: u16,
        #[arg(long, default_value_t = 40)]
        height: u16,
        #[arg(long)]
        output: PathBuf,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum ColorMode {
    Auto,
    Rgb,
    Ansi,
    Mono,
}
#[derive(Clone, Copy, ValueEnum)]
enum SnapshotScreen {
    Menu,
    Todo,
    Meadow,
    Editor,
    Details,
    Help,
    Search,
    Paused,
    LongDetails,
    Error,
    Restore,
    Ambiguous,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("windtools: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    let true_color = std::env::var("COLORTERM")
        .is_ok_and(|value| matches!(value.as_str(), "truecolor" | "24bit"));
    let options = Options {
        ascii: cli.ascii,
        mono: matches!(cli.color, ColorMode::Mono),
        ansi: matches!(cli.color, ColorMode::Ansi)
            || (matches!(cli.color, ColorMode::Auto) && !true_color),
        seed: cli.seed,
        demo: cli.demo,
    };
    if let Some(Command::Snapshot {
        screen,
        width,
        height,
        output,
    }) = &cli.command
    {
        if *width == 0 || *height == 0 || *width > 500 || *height > 200 {
            bail!("快照尺寸范围：1..500 × 1..200");
        }
        let scenario = *screen;
        let screen = match screen {
            SnapshotScreen::Menu => Screen::Menu,
            SnapshotScreen::Todo => Screen::Todo,
            SnapshotScreen::Meadow => Screen::Meadow,
            SnapshotScreen::Editor => Screen::Editor,
            SnapshotScreen::Details => Screen::Details,
            SnapshotScreen::Help
            | SnapshotScreen::Search
            | SnapshotScreen::LongDetails
            | SnapshotScreen::Restore => Screen::Todo,
            SnapshotScreen::Paused => Screen::Meadow,
            SnapshotScreen::Error | SnapshotScreen::Ambiguous => Screen::Editor,
        };
        let mut app = App::demo(
            if screen == Screen::Editor {
                Screen::Todo
            } else {
                screen
            },
            options,
        )?;
        if screen == Screen::Editor {
            app.key(ratatui::crossterm::event::KeyEvent::new(
                ratatui::crossterm::event::KeyCode::Char('a'),
                ratatui::crossterm::event::KeyModifiers::NONE,
            ));
            app.paste("核对项目文档");
            if let Some(editor) = &mut app.editor {
                editor.fields[2] = windtools::input::TextInput::new("+10m");
            }
        }
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let press = |app: &mut App, code| app.key(KeyEvent::new(code, KeyModifiers::NONE));
        match scenario {
            SnapshotScreen::Help => {
                press(&mut app, KeyCode::Char('?'));
                app.overlay_scroll = 5;
            }
            SnapshotScreen::Search => {
                press(&mut app, KeyCode::Char('/'));
                app.paste("一段很长的中文查询用于验证横向滚动和末尾的光标位置正确");
            }
            SnapshotScreen::Paused => press(&mut app, KeyCode::Char('p')),
            SnapshotScreen::LongDetails => {
                if let Some(mut task) = app.current().cloned() {
                    task.note = "这是一行很长的备注，可以上下翻看全部内容。\n".repeat(120)
                        + "长备注结尾已读";
                    app.store.update(&task)?;
                    app.reload()?;
                }
                press(&mut app, KeyCode::Enter);
                app.detail_scroll = u16::MAX;
            }
            SnapshotScreen::Error => {
                app.status("提醒日期不存在。请检查输入的月份、日期与时区，然后使用未来的 YYYY-MM-DD HH:MM，或输入 +10m。编辑内容已保留，关闭本窗口后可以继续修改。",true);
                press(&mut app, KeyCode::F(2));
            }
            SnapshotScreen::Restore => {
                app.overlay = Some(windtools::app::Overlay::Restore {
                    backup: app.store.backup()?,
                    source: "/a/long/directory/backup.json".into(),
                });
            }
            SnapshotScreen::Ambiguous => {
                if let Some(editor) = &mut app.editor {
                    editor.fields[2] = windtools::input::TextInput::new("2026-11-01 01:30");
                }
                press(&mut app, KeyCode::F(5));
            }
            _ => {}
        }
        app.meadow.tick(0.1);
        snapshot(&mut app, *width, *height, output)?;
        return Ok(());
    }
    let directory = data_directory(cli.data_dir)?;
    match &cli.command {
        Some(Command::DataPath) => {
            println!("{}", directory.display());
            return Ok(());
        }
        Some(Command::Export { path }) => {
            if cli.demo {
                bail!("导出命令不支持 --demo；请从演示界面手动导出");
            }
            Store::open(&directory)?.export(path)?;
            println!("备份已保存：{}", path.display());
            return Ok(());
        }
        Some(Command::Import { path, yes }) => {
            if cli.demo {
                bail!("恢复命令不支持 --demo；请从演示界面手动恢复");
            }
            let backup = read_backup(path)?;
            let mut store = Store::open(&directory)?;
            println!(
                "恢复预览：{} 项当前数据 -> {} 项备份数据",
                store.list()?.len(),
                backup.tasks.len()
            );
            if !yes {
                println!("未修改数据。确认后使用 --yes；恢复前会自动备份原数据。");
                return Ok(());
            }
            let safety = store.restore(&backup, chrono::Utc::now().timestamp())?;
            println!("已恢复。原数据备份：{}", safety.display());
            return Ok(());
        }
        _ => {}
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("请在交互式终端运行 TUI；导出使用 windtools export <文件路径>");
    }
    let screen = if matches!(cli.command, Some(Command::Todo)) {
        Screen::Todo
    } else if matches!(cli.command, Some(Command::Meadow)) {
        Screen::Meadow
    } else {
        Screen::Menu
    };
    let mut app = if cli.demo {
        App::demo(screen, options)?
    } else {
        App::new(Store::open(&directory)?, screen, options)?
    };
    let stopped = Arc::new(AtomicBool::new(false));
    let signal_flag = Arc::clone(&stopped);
    ctrlc::set_handler(move || {
        signal_flag.store(true, Ordering::Relaxed);
    })?;
    let mut terminal = ratatui::try_init().context("无法初始化终端")?;
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnableBracketedPaste)?;
    let mut last_frame = Instant::now();
    let interval = Duration::from_secs_f64(1.0 / f64::from(cli.fps));
    while !app.quit && !stopped.load(Ordering::Relaxed) {
        if let Err(error) = app.tick() {
            app.status(format!("提醒或数据读取失败：{error:#}"), true);
        }
        let animate = app.screen == Screen::Meadow && !app.meadow.paused && app.overlay.is_none();
        if animate && last_frame.elapsed() >= interval {
            app.meadow.tick(last_frame.elapsed().as_secs_f64());
            app.dirty = true;
            last_frame = Instant::now();
        }
        if app.dirty {
            terminal.draw(|frame| ui::render(frame, &mut app))?;
            app.dirty = false;
        }
        let wait = if animate {
            interval.saturating_sub(last_frame.elapsed())
        } else {
            Duration::from_millis(250)
        };
        if event::poll(wait)? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.key(key),
                Event::Paste(value) => app.paste(&value),
                Event::Resize(_, _) => app.dirty = true,
                _ => {}
            }
        }
    }
    Ok(())
}
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste, Show);
        ratatui::restore();
    }
}

#[derive(Serialize)]
struct Capture {
    width: u16,
    height: u16,
    cells: Vec<CaptureCell>,
}
#[derive(Serialize)]
struct CaptureCell {
    symbol: String,
    fg: String,
    bg: String,
    bold: bool,
}
fn color_hex(color: Color, foreground: bool) -> String {
    let rgb = match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Reset => {
            if foreground {
                (220, 228, 234)
            } else {
                (12, 20, 29)
            }
        }
        Color::Black => (12, 20, 29),
        Color::Red | Color::LightRed => (229, 140, 140),
        Color::Green | Color::LightGreen => (110, 160, 108),
        Color::Yellow | Color::LightYellow => (216, 190, 130),
        Color::Blue | Color::LightBlue => (64, 94, 136),
        Color::Magenta | Color::LightMagenta => (181, 135, 176),
        Color::Cyan | Color::LightCyan => (123, 200, 214),
        Color::White => (236, 241, 244),
        Color::Gray => (151, 166, 177),
        _ => (75, 92, 105),
    };
    format!("#{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
}
fn snapshot(app: &mut App, width: u16, height: u16, path: &std::path::Path) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| ui::render(frame, app))?;
    let cells = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| CaptureCell {
            symbol: cell.symbol().to_owned(),
            fg: color_hex(cell.fg, true),
            bg: color_hex(cell.bg, false),
            bold: cell.modifier.contains(ratatui::style::Modifier::BOLD),
        })
        .collect();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        serde_json::to_vec(&Capture {
            width,
            height,
            cells,
        })?,
    )?;
    Ok(())
}

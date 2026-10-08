mod details;
mod overlay;
use crate::{
    app::{App, Filter, Overlay, Screen},
    input::{TextInput, truncate},
    meadow::MeadowView,
    model::{ReminderState, Task, local_time},
};
use chrono::Utc;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

#[derive(Clone, Copy)]
struct Theme {
    muted: Color,
    accent: Color,
    warning: Color,
    error: Color,
    selection: Color,
    mono: bool,
}
impl Theme {
    fn new(app: &App) -> Self {
        if app.options.mono {
            Self {
                muted: Color::Reset,
                accent: Color::Reset,
                warning: Color::Reset,
                error: Color::Reset,
                selection: Color::Reset,
                mono: true,
            }
        } else if app.options.ansi {
            Self {
                muted: Color::Gray,
                accent: Color::Cyan,
                warning: Color::Yellow,
                error: Color::Red,
                selection: Color::Blue,
                mono: false,
            }
        } else {
            Self {
                muted: Color::Rgb(151, 166, 177),
                accent: Color::Rgb(123, 200, 214),
                warning: Color::Rgb(216, 190, 130),
                error: Color::Rgb(229, 140, 140),
                selection: Color::Rgb(30, 52, 62),
                mono: false,
            }
        }
    }
    fn selected(self) -> Style {
        if self.mono {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
                .fg(self.accent)
                .bg(self.selection)
                .add_modifier(Modifier::BOLD)
        }
    }
}
fn text(frame: &mut Frame<'_>, area: Rect, value: impl Into<String>, style: Style) {
    frame.render_widget(Paragraph::new(value.into()).style(style), area);
}
fn at(area: Rect, x: u16, y: u16, width: u16, height: u16) -> Rect {
    Rect::new(
        area.x + x.min(area.width),
        area.y + y.min(area.height),
        width.min(area.width.saturating_sub(x)),
        height.min(area.height.saturating_sub(y)),
    )
}
fn label<'a>(app: &App, cn: &'a str, en: &'a str) -> &'a str {
    if app.options.ascii { en } else { cn }
}

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let theme = Theme::new(app);
    let min_height = if app.screen == Screen::Editor { 20 } else { 14 };
    if area.width < 40 || area.height < min_height {
        text(
            frame,
            area,
            format!(
                "windtools\n{}\n{} x {}  /  {} x {}\nEsc: menu   Ctrl+C: quit",
                label(app, "请放大终端窗口", "Please enlarge the terminal"),
                area.width,
                area.height,
                40,
                min_height
            ),
            Style::new().fg(theme.warning),
        );
        return;
    }
    match app.screen {
        Screen::Menu => menu(frame, app, area, theme),
        Screen::Todo => todo(frame, app, area, theme),
        Screen::Editor => editor(frame, app, area, theme),
        Screen::Details => details::render(frame, app, area, theme),
        Screen::Meadow => meadow(frame, app, area, theme),
    }
    if app.overlay.is_some() {
        overlay::render(frame, app, area, theme);
    }
}

fn footer(frame: &mut Frame<'_>, app: &App, area: Rect, keys: &str, theme: Theme) {
    text(
        frame,
        at(area, 0, area.height - 3, area.width, 1),
        truncate(keys, area.width as usize, app.options.ascii),
        Style::new().fg(theme.muted),
    );
    let reminder = if app.options.demo {
        label(app, "演示模式", "demo")
    } else if app.scheduler_active {
        label(app, "提醒运行中", "reminders active")
    } else {
        label(app, "另一窗口负责提醒", "reminders in another window")
    };
    text(
        frame,
        at(area, 0, area.height - 1, area.width, 1),
        truncate(
            &format!("{}  |  {}", app.message, reminder),
            area.width as usize,
            app.options.ascii,
        ),
        Style::new().fg(if app.message_error {
            theme.error
        } else {
            theme.muted
        }),
    );
}
fn menu(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let width = area.width.saturating_sub(8).min(76);
    let content = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + 2,
        width,
        area.height - 6,
    );
    text(
        frame,
        at(content, 0, 0, width, 1),
        "windtools",
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    );
    text(
        frame,
        at(content, 0, 2, width, 1),
        label(
            app,
            "选择一件事，或看一阵风。",
            "A place for your tasks. A moment for the wind.",
        ),
        Style::new().fg(theme.muted),
    );
    let names = [
        label(app, "1  待办清单", "1  Todo"),
        label(app, "2  风过原野", "2  Meadow"),
    ];
    let captions = [
        format!(
            "{} {}",
            app.tasks.iter().filter(|task| !task.completed).count(),
            label(
                app,
                "项未完成 · 本地保存 · 运行中提醒",
                "open tasks / local data / session reminders"
            )
        ),
        label(
            app,
            "草海、远山、飘叶；左右键改变风向。",
            "Grass, distant hills, drifting leaves. Shape the wind.",
        )
        .into(),
    ];
    for index in 0..2 {
        let y = 4 + index as u16 * 3;
        text(
            frame,
            at(content, 0, y, width, 1),
            format!(
                "{} {}",
                if app.menu_index == index { ">" } else { " " },
                names[index]
            ),
            if app.menu_index == index {
                theme.selected()
            } else {
                Style::new()
            },
        );
        text(
            frame,
            at(content, 2, y + 1, width.saturating_sub(2), 1),
            truncate(
                &captions[index],
                width.saturating_sub(2) as usize,
                app.options.ascii,
            ),
            Style::new().fg(theme.muted),
        );
    }
    footer(
        frame,
        app,
        area,
        label(
            app,
            "↑↓ / j k 选择   Enter 进入   1/2 直达   ? 帮助   q 退出",
            "Up/Down choose   Enter open   1/2 shortcuts   ? help   q quit",
        ),
        theme,
    );
}
fn todo(frame: &mut Frame<'_>, app: &mut App, area: Rect, theme: Theme) {
    let open = app.tasks.iter().filter(|task| !task.completed).count();
    let count = format!("{} {}", open, label(app, "项未完成", "open"));
    let count_width = unicode_width::UnicodeWidthStr::width(count.as_str()) as u16;
    text(
        frame,
        at(area, 1, 0, area.width.saturating_sub(count_width + 4), 1),
        format!("windtools / {}", label(app, "待办", "todo")),
        Style::new().add_modifier(Modifier::BOLD),
    );
    text(
        frame,
        at(
            area,
            area.width.saturating_sub(count_width + 1),
            0,
            count_width,
            1,
        ),
        count,
        Style::new().fg(theme.muted),
    );
    let tabs = [Filter::Open, Filter::All, Filter::Done].map(|filter| {
        Span::styled(
            format!(
                "  {}  ",
                if app.options.ascii {
                    match filter {
                        Filter::Open => "Open",
                        Filter::All => "All",
                        Filter::Done => "Done",
                    }
                } else {
                    filter.label()
                }
            ),
            if app.filter == filter {
                theme.selected()
            } else {
                Style::new().fg(theme.muted)
            },
        )
    });
    frame.render_widget(
        Paragraph::new(Line::from(tabs.to_vec())),
        at(area, 1, 2, area.width - 2, 1),
    );
    let mut body = at(area, 1, 4, area.width - 2, area.height - 8);
    if app.searching || !app.search.text.is_empty() {
        let (visible, cursor) = app.search.viewport(body.width.saturating_sub(2) as usize);
        text(
            frame,
            at(body, 0, 0, body.width, 1),
            format!("/ {visible}"),
            Style::new().fg(theme.accent),
        );
        if app.searching {
            frame.set_cursor_position((body.x + 2 + cursor, body.y));
        }
        body = at(body, 0, 2, body.width, body.height.saturating_sub(2));
    }
    let (list_area, detail_area) = if area.width >= 100 {
        let parts = Layout::horizontal([Constraint::Percentage(64), Constraint::Percentage(36)])
            .split(body);
        (parts[0], Some(parts[1]))
    } else {
        (body, None)
    };
    if app.filtered.is_empty() {
        let message = if app.tasks.is_empty() {
            label(
                app,
                "清单还是空的。\n\n按 a 记下第一件事。",
                "Your list is empty.\n\nPress a to add your first task.",
            )
        } else if !app.search.text.is_empty() {
            label(
                app,
                "没有匹配事项。\n按 / 修改搜索；Ctrl+U 清空。",
                "No matching tasks.\nPress / to edit search, Ctrl+U to clear.",
            )
        } else {
            label(
                app,
                "这里没有事项。\nTab 切换视图，a 新增。",
                "No tasks in this view.\nTab changes view; a adds a task.",
            )
        };
        text(
            frame,
            at(
                list_area,
                2,
                1,
                list_area.width.saturating_sub(4),
                list_area.height.saturating_sub(1),
            ),
            message,
            Style::new().fg(theme.muted),
        );
    } else {
        let visible = (list_area.height / 2).max(1) as usize;
        if app.selected < app.scroll {
            app.scroll = app.selected;
        }
        if app.selected >= app.scroll + visible {
            app.scroll = app.selected + 1 - visible;
        }
        for (row, index) in app
            .filtered
            .iter()
            .enumerate()
            .skip(app.scroll)
            .take(visible)
        {
            let task = &app.tasks[*index];
            let selected = row == app.selected;
            let y = ((row - app.scroll) * 2) as u16;
            let check = if task.completed { "[x]" } else { "[ ]" };
            text(
                frame,
                at(list_area, 0, y, list_area.width.saturating_sub(2), 1),
                format!(
                    "{} {} {}",
                    if selected { ">" } else { " " },
                    check,
                    truncate(
                        &task.title,
                        list_area.width.saturating_sub(9) as usize,
                        app.options.ascii
                    )
                ),
                if selected {
                    theme.selected()
                } else if task.completed {
                    Style::new().fg(theme.muted)
                } else {
                    Style::new()
                },
            );
            let reminder = reminder_text(task, app.options.ascii);
            text(
                frame,
                at(list_area, 6, y + 1, list_area.width.saturating_sub(8), 1),
                truncate(
                    &reminder,
                    list_area.width.saturating_sub(8) as usize,
                    app.options.ascii,
                ),
                Style::new().fg(
                    if matches!(
                        task.reminder_state,
                        ReminderState::Missed | ReminderState::Failed
                    ) {
                        theme.warning
                    } else {
                        theme.muted
                    },
                ),
            );
        }
    }
    if let Some(detail_area) = detail_area
        && let Some(task) = app.current()
    {
        task_detail(frame, app, task, detail_area, theme);
    }
    footer(
        frame,
        app,
        area,
        if area.width < 60 {
            label(
                app,
                "a新增 e编辑 Space完成 /搜索 ?帮助",
                "a add e edit Space done / find ? help",
            )
        } else if area.width < 78 {
            label(
                app,
                "a 新增  e 编辑  Space 完成  / 搜索  ? 帮助",
                "a add  e edit  Space done  / search  ? help",
            )
        } else {
            label(
                app,
                "a 新增  e 编辑  Space 完成  d 删除  / 搜索  Tab 视图  b 备份  i 恢复  Esc 菜单",
                "a add  e edit  Space done  d delete  / search  Tab view  b backup  i restore  Esc menu",
            )
        },
        theme,
    );
}
fn reminder_text(task: &Task, ascii: bool) -> String {
    if task.reminder_state == ReminderState::Pending {
        task.remind_at
            .map(|at| local_time(at, "%m-%d %H:%M"))
            .unwrap_or_default()
    } else if ascii {
        match task.reminder_state {
            ReminderState::None => "No reminder",
            ReminderState::Sending => "Submitting",
            ReminderState::Sent => "Submitted to system",
            ReminderState::Missed => "Reminder missed",
            ReminderState::Failed => "Send failed",
            ReminderState::Cancelled => "Reminder cancelled",
            ReminderState::Pending => "Scheduled",
        }
        .into()
    } else {
        task.reminder_state.label().into()
    }
}
fn task_detail(frame: &mut Frame<'_>, app: &App, task: &Task, area: Rect, theme: Theme) {
    let inner = at(area, 2, 0, area.width.saturating_sub(4), area.height);
    text(
        frame,
        at(inner, 0, 0, inner.width, 1),
        label(app, "事项详情", "Details"),
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    );
    let mut lines = vec![
        Line::raw(""),
        Line::raw(task.title.clone()),
        Line::raw(""),
        Line::styled(
            reminder_text(task, app.options.ascii),
            Style::new().fg(theme.warning),
        ),
    ];
    if let Some(time) = task.remind_at {
        lines.push(Line::styled(
            local_time(time, "%Y-%m-%d %H:%M:%S %:z"),
            Style::new().fg(theme.muted),
        ));
    }
    lines.push(Line::raw(""));
    lines.extend(task.note.lines().map(|line| Line::raw(line.to_owned())));
    if let Some(error) = &task.last_error {
        lines.push(Line::raw(""));
        lines.push(Line::styled(error.clone(), Style::new().fg(theme.error)));
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        at(inner, 0, 1, inner.width, inner.height.saturating_sub(1)),
    );
}
fn field(
    frame: &mut Frame<'_>,
    input: &TextInput,
    area: Rect,
    title: &str,
    focused: bool,
    theme: Theme,
) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .title(title)
        .border_style(Style::new().fg(if focused { theme.accent } else { theme.muted }));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let (visible, cursor) = input.viewport(inner.width.saturating_sub(1) as usize);
    text(frame, inner, visible, Style::new());
    if focused && inner.width > 0 && inner.height > 0 {
        frame.set_cursor_position((inner.x + cursor, inner.y));
    }
}
fn editor(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let Some(editor) = &app.editor else {
        return;
    };
    let width = area.width.saturating_sub(6).min(90);
    let inner = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + 1,
        width,
        area.height - 5,
    );
    text(
        frame,
        at(inner, 0, 0, width, 1),
        if editor.original.is_some() {
            label(app, "编辑事项", "Edit task")
        } else {
            label(app, "新增事项", "Add task")
        },
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    );
    let labels = [
        label(app, "标题 *", "Title *"),
        label(app, "备注", "Note"),
        label(app, "提醒时间（留空取消）", "Reminder (empty = none)"),
    ];
    for (index, title) in labels.iter().enumerate() {
        field(
            frame,
            &editor.fields[index],
            at(inner, 0, 2 + index as u16 * 3, width, 3),
            title,
            editor.focus == index,
            theme,
        );
    }
    let time_input = editor.fields[2].text.trim();
    let preview = if time_input.is_empty() {
        label(
            app,
            "未设置提醒；输入日期或 +10m",
            "No reminder; enter a date or +10m",
        )
        .into()
    } else if crate::model::ambiguous_reminder_candidates(time_input).is_some() {
        label(
            app,
            "此时间有两种解释，F5 明确选择",
            "Ambiguous time: F5 chooses an offset",
        )
        .into()
    } else if time_input == editor.original_time {
        editor
            .original
            .as_ref()
            .and_then(|task| task.remind_at)
            .map(|at| local_time(at, "%Y-%m-%d %H:%M:%S %:z"))
            .unwrap_or_default()
    } else {
        match crate::model::parse_reminder(time_input, Utc::now().timestamp()) {
            Ok(Some(at)) => local_time(at, "%Y-%m-%d %H:%M:%S %:z"),
            Ok(None) => String::new(),
            Err(error) => format!("{error:#}"),
        }
    };
    text(
        frame,
        at(inner, 0, 12, width, 1),
        truncate(&preview, width as usize, app.options.ascii),
        Style::new().fg(theme.warning),
    );
    text(
        frame,
        at(inner, 0, 13, width, 1),
        label(
            app,
            "YYYY-MM-DD HH:MM / +10m  Ctrl+U 清空",
            "YYYY-MM-DD HH:MM / +10m  Ctrl+U clears",
        ),
        Style::new().fg(theme.muted),
    );
    if app.message_error {
        text(
            frame,
            at(inner, 0, 14, width, 1),
            label(
                app,
                "保存失败；F2 查看完整原因",
                "Save failed; F2 shows full details",
            ),
            Style::new().fg(theme.error),
        );
    }
    footer(
        frame,
        app,
        area,
        if area.width < 70 {
            label(
                app,
                "Tab 字段 Ctrl+S 保存 F2 错误 Esc 返回",
                "Tab field Ctrl+S save F2 error Esc back",
            )
        } else {
            label(
                app,
                "Tab / Enter 下一字段   Ctrl+S 保存   F2 完整错误   Esc 返回",
                "Tab / Enter next field   Ctrl+S save   F2 error details   Esc back",
            )
        },
        theme,
    );
}
fn meadow(frame: &mut Frame<'_>, app: &mut App, area: Rect, theme: Theme) {
    let art = at(area, 0, 0, area.width, area.height - 3);
    app.meadow.prepare(art.width, art.height);
    frame.render_widget(
        MeadowView {
            meadow: &app.meadow,
            ascii: app.options.ascii,
            mono: app.options.mono,
            ansi: app.options.ansi,
        },
        art,
    );
    text(
        frame,
        at(area, 1, area.height - 3, area.width - 2, 1),
        if app.message_error {
            truncate(
                &app.message,
                area.width.saturating_sub(2) as usize,
                app.options.ascii,
            )
        } else if app.options.demo {
            label(app, "演示模式，不发送通知", "Demo: no notifications").into()
        } else if app.scheduler_active {
            label(app, "提醒运行中", "Reminders active").into()
        } else {
            label(app, "另一窗口负责提醒", "Another window handles reminders").into()
        },
        Style::new().fg(if app.message_error {
            theme.error
        } else {
            theme.muted
        }),
    );
    let direction = if app.meadow.direction > 0.0 {
        label(app, "向右", "right")
    } else {
        label(app, "向左", "left")
    };
    text(
        frame,
        at(area, 1, area.height - 2, area.width - 2, 1),
        format!(
            "{} {} / {} / seed {}",
            label(app, "风过原野", "Meadow"),
            if app.meadow.paused {
                label(app, "[已暂停]", "[paused]")
            } else {
                ""
            },
            direction,
            app.meadow.seed
        ),
        Style::new().fg(theme.accent),
    );
    text(
        frame,
        at(area, 1, area.height - 1, area.width - 2, 1),
        truncate(
            if area.width < 70 {
                label(
                    app,
                    "←→ 风向 Space 阵风 p 暂停 Esc 菜单",
                    "Arrows wind Space gust p pause Esc menu",
                )
            } else {
                label(
                    app,
                    "← → 风向   Space 阵风   p 暂停   Esc 菜单",
                    "Left/Right wind   Space gust   p pause   Esc menu",
                )
            },
            area.width.saturating_sub(2) as usize,
            app.options.ascii,
        ),
        Style::new().fg(theme.muted),
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Options;
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn narrow_help_and_long_details_are_reachable_by_scrolling() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = App::demo(Screen::Todo, Options::default()).unwrap();
        let press = |app: &mut App, code| app.key(KeyEvent::new(code, KeyModifiers::NONE));
        press(&mut app, KeyCode::Char('?'));
        let mut terminal = Terminal::new(TestBackend::new(40, 14)).unwrap();
        let mut found_restore = false;
        for _ in 0..20 {
            let completed = terminal.draw(|frame| render(frame, &mut app)).unwrap();
            let dump = completed
                .buffer
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .replace(' ', "");
            found_restore |= dump.contains("导入恢复");
            press(&mut app, KeyCode::Down);
        }
        assert!(found_restore, "help recovery action was not reachable");
        press(&mut app, KeyCode::Esc);
        let mut task = app.current().unwrap().clone();
        task.note = "长内容\n".repeat(100) + "结尾标记";
        app.store.update(&task).unwrap();
        app.reload().unwrap();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::End);
        let completed = terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let dump = completed
            .buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .replace(' ', "");
        assert!(dump.contains("结尾标记"));
    }
    #[test]
    fn search_tail_pause_and_full_error_remain_visible() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = App::demo(Screen::Todo, Options::default()).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
        app.search = TextInput::new("测试长查询内容".repeat(10) + "末尾");
        app.searching = true;
        app.refilter();
        let completed = terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let dump = completed
            .buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .replace(' ', "");
        assert!(dump.contains("末尾"));
        app.searching = false;
        app.screen = Screen::Meadow;
        app.meadow.paused = true;
        let completed = terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let dump = completed
            .buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .replace(' ', "");
        assert!(dump.contains("已暂停"));
        assert!(dump.contains("不发送通知"), "{dump}");
        app.status("完整原因\n".repeat(30) + "修正方法", true);
        app.key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE));
        app.key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        let completed = terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let dump = completed
            .buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .replace(' ', "");
        assert!(dump.contains("修正方法"));
    }
    #[test]
    fn all_screens_render_at_supported_sizes_without_losing_task_text() {
        for (width, height) in [(40, 14), (78, 24), (120, 40), (240, 80)] {
            for screen in [Screen::Menu, Screen::Todo, Screen::Meadow, Screen::Details] {
                let mut app = App::demo(
                    screen,
                    Options {
                        seed: 42,
                        ..Options::default()
                    },
                )
                .unwrap();
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal.draw(|frame| render(frame, &mut app)).unwrap();
                let dump = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(!dump.trim().is_empty());
                if screen == Screen::Todo {
                    assert!(dump.replace(' ', "").contains("提交材料"));
                }
            }
        }
    }
}

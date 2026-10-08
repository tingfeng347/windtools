use super::*;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(super) fn wrap(value: &str, width: u16) -> Vec<String> {
    let mut lines = Vec::new();
    let limit = usize::from(width.max(1));
    for source in value.split('\n') {
        let mut line = String::new();
        let mut used = 0;
        for grapheme in source.graphemes(true) {
            let size = UnicodeWidthStr::width(grapheme);
            if used + size > limit && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }
            line.push_str(grapheme);
            used += size;
        }
        lines.push(line);
    }
    lines
}

pub(super) fn render(frame: &mut Frame<'_>, app: &mut App, area: Rect, theme: Theme) {
    let width = area.width.saturating_sub(4);
    let height = area.height.saturating_sub(7);
    if let Some(task) = app.current() {
        let mut content = format!(
            "{}\n\n{}\n",
            task.title,
            reminder_text(task, app.options.ascii)
        );
        if let Some(at) = task.remind_at {
            content.push_str(&format!("{}\n", local_time(at, "%Y-%m-%d %H:%M:%S %:z")));
        }
        content.push('\n');
        content.push_str(&task.note);
        if let Some(error) = &task.last_error {
            content.push_str(&format!(
                "\n\n{}\n{error}",
                label(app, "通知错误", "Notification error")
            ));
        }
        let lines = wrap(&content, width);
        let max = lines.len().saturating_sub(height as usize);
        app.detail_scroll = (app.detail_scroll as usize).min(max) as u16;
        let start = app.detail_scroll as usize;
        let visible = lines
            .iter()
            .skip(start)
            .take(height as usize)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        text(
            frame,
            at(area, 2, 1, width, 1),
            format!(
                "{}  {}/{}",
                label(app, "事项详情", "Details"),
                start + 1,
                lines.len()
            ),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        );
        text(frame, at(area, 2, 3, width, height), visible, Style::new());
    }
    footer(
        frame,
        app,
        area,
        label(
            app,
            "↑↓ 翻看  PgUp/PgDn 翻页  e 编辑  Esc 返回",
            "Up/Down scroll PgUp/PgDn e edit Esc back",
        ),
        theme,
    );
}

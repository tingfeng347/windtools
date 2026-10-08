use super::*;

pub(super) fn render(frame: &mut Frame<'_>, app: &mut App, area: Rect, theme: Theme) {
    let width = area.width.saturating_sub(4).min(80);
    let height = area.height.saturating_sub(2).min(
        if matches!(app.overlay, Some(Overlay::Help | Overlay::Error(_))) {
            24
        } else {
            15
        },
    );
    let dialog = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, dialog);
    let block = Block::bordered().border_style(Style::new().fg(theme.accent));
    let content = block.inner(dialog);
    frame.render_widget(block, dialog);
    let inner = at(
        content,
        1,
        0,
        content.width.saturating_sub(2),
        content.height,
    );
    let Some(overlay) = &app.overlay else {
        return;
    };
    let (title,body,keys)=match overlay{
        Overlay::Help=>(label(app,"键盘操作","Keyboard"),label(app,"菜单：1 待办，2 原野，Enter 进入\n待办：a 新增，e 编辑\nSpace 完成/恢复，d 删除\n↑↓ 或 j/k 移动，Home/End\nPgUp/PgDn 翻页\nTab 视图，/ 搜索，Ctrl+U 清空\nEnter 详情；详情中 ↑↓ 翻看\nb 导出备份，i 导入恢复\n表单：Tab 字段，Ctrl+S 保存\nF2 完整错误；歧义时间 F5 选择\n粘贴可输入多行备注与中文\n原野：左右键风向，Space 阵风\np 暂停，Esc 菜单，q 退出\n\n提醒覆盖会话，错过不补发。","Menu: 1 todo, 2 meadow, Enter open\nTodo: a add, e edit, Space done\nd delete, b backup, i restore\nUp/Down or j/k, Home/End\nPgUp/PgDn pages, Tab view\n/ search, Ctrl+U clears\nEnter details; arrows scroll\nForm: Tab fields, Ctrl+S saves\nF2 full error, F5 time choices\nPaste supports multiline notes\nMeadow: arrows wind, Space gust\np pause, Esc menu, q quit\n\nReminders run across the session.\nMissed reminders never replay.").to_owned(),label(app,"↑↓ 翻看  Enter/Esc 关闭","Up/Down scroll Enter/Esc close")),
        Overlay::Error(error)=>(label(app,"完整错误信息","Error details"),error.clone(),label(app,"↑↓ 翻看  Enter/Esc 返回","Up/Down scroll Enter/Esc back")),
        Overlay::TimeChoice(values)=>(label(app,"选择提醒时间","Choose reminder time"),format!("{}\n\n1 {}\n2 {}",label(app,"此本地时间有两种解释，请明确选择。","This local time occurs twice. Choose an offset."),local_time(values[0],"%Y-%m-%dT%H:%M:%S%:z"),local_time(values[1],"%Y-%m-%dT%H:%M:%S%:z")),label(app,"1/2 选择  Esc 保留输入","1/2 choose  Esc keep input")),
        Overlay::Delete(task)=>(label(app,"删除事项？","Delete task?"),format!("{}\n\n{}",label(app,"将取消尚未发送的提醒。","Unsent reminders will be cancelled."),task.title),label(app,"y 删除  n/Esc 取消","y delete n/Esc cancel")),
        Overlay::Discard{exit}=>(label(app,"放弃未保存的修改？","Discard unsaved changes?"),if *exit{label(app,"放弃当前输入并退出；取消可继续编辑。","Discard the input and quit? Cancel to keep editing.")}else{label(app,"放弃当前输入并返回清单？","Discard the input and return to the list?")}.into(),label(app,"y 放弃  n/Esc 继续编辑","y discard n/Esc keep editing")),
        Overlay::Export(_)=>(label(app,"导出备份","Export backup"),label(app,"包含所有事项，不覆盖已有文件。","Includes all tasks; existing files are preserved.").into(),label(app,"Enter 导出  Esc 取消","Enter export Esc cancel")),
        Overlay::Import(_)=>(label(app,"打开备份","Open backup"),label(app,"输入 JSON 路径，然后预览恢复。","Enter a JSON path to preview the restore.").into(),label(app,"Enter 预览  Esc 取消","Enter preview Esc cancel")),
        Overlay::Restore{backup,source}=>(label(app,"确认恢复备份？","Restore backup?"),format!("{}\n{}\n\n{} {} -> {} {}\n\n{}",label(app,"替换清单前自动备份原数据。","A safety backup precedes replacing the list."),label(app,"过去的提醒不会补发。","Past reminders will not replay."),app.tasks.len(),label(app,"当前","current"),backup.tasks.len(),label(app,"备份","backup"),source),label(app,"y 恢复(先备份)  n/Esc 取消","y restore (backup first) n/Esc cancel")),
    };
    let path_input = matches!(overlay, Overlay::Export(_) | Overlay::Import(_));
    text(
        frame,
        at(inner, 0, 0, inner.width, 1),
        title,
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    );
    if path_input {
        text(
            frame,
            at(inner, 0, 2, inner.width, 1),
            truncate(&body, inner.width as usize, app.options.ascii),
            Style::new().fg(theme.muted),
        );
        if let Overlay::Export(input) | Overlay::Import(input) = overlay {
            field(
                frame,
                input,
                at(inner, 0, 4, inner.width, 3),
                label(app, "文件路径", "File path"),
                true,
                theme,
            );
        }
    } else {
        let body_area = at(inner, 0, 2, inner.width, inner.height.saturating_sub(4));
        let lines = details::wrap(&body, body_area.width);
        app.overlay_scroll = (app.overlay_scroll as usize)
            .min(lines.len().saturating_sub(body_area.height as usize))
            as u16;
        let visible = lines
            .into_iter()
            .skip(app.overlay_scroll as usize)
            .take(body_area.height as usize)
            .collect::<Vec<_>>()
            .join("\n");
        text(frame, body_area, visible, Style::new());
    }
    let keys = if inner.width < 36 && matches!(app.overlay, Some(Overlay::Restore { .. })) {
        label(app, "y 恢复并备份  n/Esc 取消", "y restore n/Esc cancel")
    } else {
        keys
    };
    text(
        frame,
        at(inner, 0, inner.height.saturating_sub(1), inner.width, 1),
        truncate(keys, inner.width as usize, app.options.ascii),
        Style::new().fg(theme.warning),
    );
    if path_input && app.message_error {
        text(
            frame,
            at(inner, 0, inner.height.saturating_sub(3), inner.width, 1),
            label(
                app,
                "操作失败，F2 查看完整错误",
                "Failed: F2 shows full error",
            ),
            Style::new().fg(theme.error),
        );
    }
}

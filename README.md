# windtools

一个 Rust 终端工具集，首版包含本地待办清单和可互动的字符原野。

## 启动

需要 Rust 1.89 或更新版本。

```sh
cargo run --release
cargo run --release -- todo
cargo run --release -- meadow
```

安装为本地命令：

```sh
cargo install --path . --locked
windtools
windtools todo
windtools meadow
```

`windtools` 打开统一选择菜单。运行 `windtools --demo todo` 可体验临时演示清单，不读取真实数据，不发送通知；演示数据退出后丢弃。

## 待办

| 操作 | 按键 |
| --- | --- |
| 新增 / 编辑 | `a` / `e` |
| 完成或恢复未完成 | `Space` |
| 移动 / 首末 / 翻页 | `↑↓`、`j/k`、`Home/End`、`PgUp/PgDn` |
| 切换未完成、全部、已完成 | `Tab` |
| 搜索标题 | `/`，`Enter` 结束输入，`Ctrl+U` 清空 |
| 查看详情 | `Enter` |
| 翻看长详情、帮助和错误 | `↑↓`、`PgUp/PgDn`、`Home/End` |
| 删除（需确认） | `d` |
| 导出备份 / 恢复备份 | `b` / `i` |
| 帮助 / 返回菜单 / 退出 | `?` / `Esc` / `q` 或 `Ctrl+C` |

表单用 `Tab` 或 `Enter` 切换字段，`Ctrl+S` 保存；支持中文输入和粘贴。`Esc` 返回，有未保存修改时会询问是否放弃。保存失败时，`F2` 可查看并滚动完整错误，关闭后继续编辑。

提醒时间支持 `2026-10-08 15:00`、`+10m`、`+2h` 等输入，表单会显示实际日期预览。遇到夏令时歧义时，按 `F5` 查看两种真实时刻并明确选择，或使用带时区偏移的格式，例如 `2026-11-01T01:30:00-04:00`。

事项每次修改立即提交到 SQLite。通知覆盖整个 TUI 会话，在菜单或原野中仍然运行；退出程序后停止。关闭程序或休眠期间错过的提醒不会补发，也不会自动完成或删除事项。界面保留“已错过”状态，重新设置未来时间可再次提醒。

完成或删除事项会取消尚未提交的提醒。已向操作系统提交的通知不能被撤回；恢复未完成状态不会自动重发旧提醒。发送结果区分“已提交系统”与“发送失败”；系统提交成功不能证明用户已经看到通知。

多个窗口共用清单，由一个窗口负责通知调度。其他窗口继续支持编辑；当调度窗口退出后，其他窗口会接手。为了跳过休眠/进程暂停期间的提醒，连续调度中断超过 5 秒会被视为错过区间（包括调试器暂停或长时间系统阻塞）。

## 数据与备份

查看实际位置：

```sh
windtools data-path
windtools --data-dir ./my-data todo
windtools export ./backup.json
windtools import ./backup.json
windtools import ./backup.json --yes
```

默认使用用户数据目录：Linux 的 XDG 数据目录、macOS 的 Application Support、Windows 的 Local AppData。主文件为 `windtools.sqlite3`，备份为带版本的 JSON。

导出不会覆盖已有文件。导入默认只预览；TUI 中再次确认，或命令行传入 `--yes` 后恢复。恢复替换当前清单，并先在数据目录的 `backups/` 中保存原数据；非法备份不会修改清单。

## 风过原野

`←` / `→` 调整风向，`Space` 触发阵风，`p` 暂停，`Esc` 返回菜单。固定镜头包含远山、草海和飘叶，使用有数量上限的字符元素。

```sh
windtools meadow --seed 42 --fps 30
windtools meadow --ascii --color ansi
windtools todo --color mono
```

`--ascii` 使用 ASCII 景观字符和英文操作标签；事项内容、用户输入和错误信息仍可包含 Unicode。`--color` 支持 `auto`、`rgb`、`ansi`、`mono`；普通等宽字体即可使用。待办最小窗口为 40×14，表单为 40×20，宽度不足 100 时采用独立详情页。

系统通知依赖当前操作系统的桌面通知能力与用户设置。Linux 的 SSH/无桌面会话可使用清单，通知服务不存在时会显示发送失败。macOS/Windows 上的通知来源名称由系统后端决定，不保证独立应用图标。

## 验证

```sh
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
cargo bench --bench workloads --locked
```

三平台 CI 构建与实际终端/桌面通知验证分别记录。当前实测结果见 [验证记录](docs/validation.md)；设计依据见 [首版规格](docs/spec-v1.md)。

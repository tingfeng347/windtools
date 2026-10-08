# 本地 TUI 待办工具小范围调研

日期：2026-10-07。服务于 windtools 首发实用工具：TUI 待办、本地持久化、可通知；已定 Windows/macOS/Linux 原生与 Unicode/ASCII。观赏工具已选“风过原野”。本次只核对以下三个产品方向，使用 Tavily 搜索、提取官方文档和源码；未安装、未运行、未测试通知。资料未说明某功能，不代表该功能不存在。

| 产品 | 独立运行与本地数据：已核实事实 | 提醒及后台：本次证据边界 |
| --- | --- | --- |
| Taskwarrior / taskwarrior-tui | TUI 是 Taskwarrior 的界面；其 README 明确要求先安装 Taskwarrior，并共用 `.taskrc`。Taskwarrior 默认数据目录配置为 `~/.task`；3.x 在任务目录中使用 `taskchampion.sqlite3`。[TUI 官方说明](https://github.com/kdheepak/taskwarrior-tui)、[数据目录配置](https://taskwarrior.org/docs/configuration/)、[3.x 存储说明](https://taskwarrior.org/docs/upgrade-3/) | 本次所读 TUI README 与存储文档未证实内置系统通知，亦未证实退出 TUI 后自动提醒。不能把截止日期、任务查询或外部集成能力等同于后台提醒。 |
| Dooit | 可直接启动的 Python/Textual TUI，强调可定制界面、Vim 键位与分支清单。当前入口源码支持 `--db` 指定数据库文件、`--config` 指定配置；不应把 2023 年讨论中的 YAML 存储当作现状，CHANGELOG 已说明 v3 移除 YAML 支持。[项目](https://github.com/dooit-org/dooit)、[当前入口源码](https://github.com/dooit-org/dooit/blob/main/dooit/__main__.py)、[变更记录](https://github.com/dooit-org/dooit/blob/main/CHANGELOG.md) | 本次读取的官方资料未证实系统通知或后台提醒。未进一步确认数据库类型与默认路径；不据此作推断。 |
| tuxedo | 更直接的 Rust 本地参照：一个二进制同时提供 TUI 与 CLI；读写标准 `todo.txt`，每次修改采用临时文件再重命名保存；官方列出 Linux/macOS/Windows 发布目标。README 明确描述无 daemon、无数据库、无云端。[官方 README](https://github.com/webstonehq/tuxedo) | “无 daemon”是作者明确描述；本次资料未证实系统通知和退出后提醒，不能由“无 daemon”进一步断言不可能通过系统调度实现通知。 |

## 对 windtools 的机会假设

**假设：用户需要一款无需另装任务引擎的轻量本地清单，从一条输入完成“事项＋提醒时间”，并能明确看见提醒是否已经安排、送出或错过。** 后续访谈已明确待办采用独立风格，与观赏工具共用启动入口。

这是假设，不是已证明的竞争空白：上表已经有独立、本地、Rust、三平台产品，不能只把这些实现属性当卖点。需要用户确认提醒的使用场景及生命周期，才能判断比已有清单多解决了什么问题。

## MVP 范围与风险

- 建议基础闭环：添加、编辑、完成、删除/撤销；简单筛选；事项与一次性提醒时间；可靠本地保存；可解释的提醒结果。数据格式、分组、优先级等仍待设计，不因竞品具备就照单加入。
- **用户已选择仅在 TUI 运行时发送系统通知。** 首版由前台进程调度，不需要常驻后台或操作系统定时任务；相关边界已写入 `docs/adr/0002-reminders-while-tui-runs.md`。
- 提醒必须定义完成/删除后取消、编辑时间后重新安排、重复投递防护、系统休眠后的错过提醒处理，以及通知被系统禁止时怎样反馈。不能把“调用通知 API 成功”当成用户已经看到通知。
- 数据可靠性需要考虑写入失败、异常退出、备份/迁移和多个实例同时打开；“本地持久化”不只是退出时写一个文件。
- 首版若再加入循环任务、云同步、共享、任意通知渠道或复杂项目层级，容易把轻量清单扩为任务管理平台。是否纳入必须有具体使用场景支撑。

用户已确认错过的提醒不再补发。单次待办、本机统一清单、导出备份及独立视觉风格也已确定，最新决定以 `docs/design.md` 为准。

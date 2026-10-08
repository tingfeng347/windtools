# windtools 首版验证记录

日期：2026-10-08。环境：Intel macOS 14.8.9，Rust/Cargo 1.98.0。所有测试清单均使用临时目录或内存数据库，没有读取个人清单。

## 自动检查

以下检查通过：

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
```

15 项测试覆盖 UTF-8/字素编辑、提醒时间解析、立即保存、过期版本冲突、重复实例领取与取消、错过和中断不补发、恢复前备份、无效备份拒绝、会话跨页面提醒及失败反馈、写入失败保留输入、暂停与 ASCII 景观，以及窄屏搜索、详情、帮助和错误的可达性。

## 终端和通知

`python3 scripts/pty_smoke.py` 通过真实伪终端运行事件循环，验证中文粘贴、新增保存、切换菜单/原野、40×14 调整窗口、完成取消提醒、退出重开、JSON 导出与导入预览，以及退出时离开备用屏幕并关闭括号粘贴。

`python3 scripts/native_notification.py` 在临时数据目录创建一个标记为测试的提醒，在原野页面等到到期。macOS 通知调用成功，记录为 `sent`（界面“已提交系统”），无后端错误。没有人工确认桌面横幅或通知中心可见，不能据此宣称用户收到通知。

界面证据位于 `.impeccable/review/`：120×40 和 40×14 清单、菜单、编辑表单、120×40/40×14/240×80 原野；另有长搜索、暂停、可滚动帮助/详情/错误、恢复警告和夏令时歧义选择。它们来自实际 Ratatui 单元格缓冲区，经 QA 脚本渲染为 PNG，包含合成演示数据；不是终端模拟器的系统截图，不证明字体或通知的实际桌面表现。

## Release 性能

`cargo bench --bench workloads --locked` 使用 10,000 件合成待办，原野逐帧采样。下列是一次本机运行结果，不是性能保证：

| 测量 | 结果 |
| --- | ---: |
| 加载 10,000 件事项 | 21.584 ms |
| UI 搜索过滤平均 | 2.058 ms |
| 原野 120×40 绘制 p50 / p95 | 0.092 / 0.097 ms |
| 原野 240×80 绘制 p50 / p95 | 0.253 / 0.298 ms |
| 清单 120×40 / 240×80 平均绘制 | 0.174 / 0.523 ms |

绘制测量使用内存终端后端，不包含终端模拟器渲染或键盘到屏幕延迟。

`python3 scripts/measure_pty.py` 另以真实 PTY 运行 release 进程，加载 10,000 件事项，30 FPS 原野，每档采样约 5 秒：

| 页面 / 尺寸 | ps CPU 样本 | RSS | 稳态输出 |
| --- | ---: | ---: | ---: |
| 清单 / 120×40 | 0.0% | 10.30 MiB | 0 B/s |
| 原野 / 120×40 | 1.4% | 11.10 MiB | 18,655 B/s |
| 原野 / 240×80 | 3.9% | 11.53 MiB | 46,545 B/s |

PTY 收集字节但不绘制画面；CPU/RSS 是应用进程样本，包含其线程，不包含终端模拟器。原始本机记录在被 git 忽略的 `artifacts/pty-performance.json`、`artifacts/native-notification.json` 和 `artifacts/pty-smoke.ansi`，可用脚本重测。

## 尚未验收

已配置 GitHub Actions 的 Linux/macOS/Windows 构建、测试和可执行文件产物矩阵，尚未远端运行。Linux/Windows 原生键盘交互和桌面通知、真实 SSH/tmux、不同终端字体、100 ms 输入响应目标及长期资源稳定性尚未实测。当前可运行交付以本机 macOS 的证据为准。

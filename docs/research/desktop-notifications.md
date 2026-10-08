# 运行中的 TUI 发送桌面通知

核对日期：2026-10-07。用户已选择仅在 TUI 运行时通知，因此本报告只讨论即时发送。以下为官方文档与源码核对，macOS 的系统提交已在实现后实测，Linux/Windows 尚未实际发送；桌面可见性另行人工验收。

## 平台事实

- `notify-rust` 当前 macOS 默认后端是 `mac-notification-sys`；`mac-usernotifications` 是非默认 `preview-macos-un` 功能。后者要求应用 bundle 与签名，不能把此要求推广到所有 macOS 通知方式。[依赖声明](https://github.com/hoodie/notify-rust/blob/main/Cargo.toml)、[现代后端说明](https://docs.rs/mac-usernotifications/latest/mac_usernotifications/)
- macOS 可以从终端经 `osascript` 执行 AppleScript 的 `display notification`。通知是否展示受系统设置控制，命令返回不能证明用户已看到；标题只是通知内容，发送者归属脚本或运行宿主，实际展示需实测。[终端执行方式](https://support.apple.com/en-lb/guide/terminal/trml1003/mac)、[通知指南](https://developer.apple.com/library/archive/documentation/LanguagesUtilities/Conceptual/MacAutomationScriptingGuide/DisplayNotifications.html)
- Linux 依赖用户桌面会话中的通知服务；通知协议明确不能假设服务总存在。因此 SSH 或无桌面环境中的可运行 TUI，不等于可展示系统弹窗。[freedesktop 规范](https://specifications.freedesktop.org/notification/latest-single/)
- `notify-rust` 的 Windows 路径目前使用 `tauri-winrt-notification`，默认发送方式涉及 PowerShell App ID。独立应用身份需要额外注册与安装设计，不能只靠修改通知标题实现。[实现](https://github.com/hoodie/notify-rust/blob/main/src/windows.rs)、[底层库说明](https://docs.rs/tauri-winrt-notification/latest/tauri_winrt_notification/struct.Toast.html)

## 实现选择（2026-10-08）

采用小范围的通知适配接口，只发送标题和正文。Linux/Windows 使用锁定的 `notify-rust 4.18.2`；macOS 使用系统 `osascript`，检查进程退出结果并设 5 秒超时。用户文字作为参数传递，不插入 AppleScript 源码。

实现时检查了锁定依赖的 macOS 后端：旧后端在释放通知句柄时发送且不向调用者传播发送错误，因此没有用于需要明确失败反馈的路径。最终实现见 `src/notifier.rs`。不承诺独立的 windtools 图标或权限项。

TUI 内部调度提醒，通知操作与列表交互分离，失败时仍可查看事项并看到错误状态。区分“已向系统提交”与“用户已阅读”；首版不以点击通知完成事项、必达通知或精确一次交付为验收标准。

运行中到期、休眠恢复、重新启动后的错过提醒，以及重复实例的提交行为，需要按设计记录中的规则实现和测试。测试包含成功提交、后端缺失、调用失败与界面正常退出，桌面实际显示另外人工验证。

本机一次性测试在原野页面等到提醒，状态变为“已提交系统”，没有错误。此证据仅证明调用成功，未确认通知中心/横幅实际可见；详见 `docs/validation.md`。

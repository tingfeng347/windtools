# windtools

<!-- impeccable:product-schema 1 -->

## Platform

跨平台终端 TUI：macOS、Linux、Windows。此项目不是网页或移动应用。

## Stack

用户指定 Rust，并确认 `docs/spec-v1.md` 的终端字符绘制方案。

## Users

日常使用终端的开发者，以及喜欢字符艺术的终端玩家。

## Product Purpose

一个统一入口提供本地待办清单和可互动的原野字符景观。待办用于记录、查看、完成事项和接收运行中提醒；原野用于观赏与把玩。

## Capabilities and Constraints

本机统一的单次待办清单，本地保存、导出与恢复。提醒在整个 windtools TUI 会话中调度，退出或休眠期间错过的提醒不补发。允许 Unicode，提供 ASCII 和单色模式，普通等宽字体即可运行。首发两款工具的视觉风格不同。

## Evidence on Hand

用户已完成设计访谈，并于 2026-10-08 确认首版规格和开始实现。依据：`docs/spec-v1.md`、`docs/design.md`、`CONTEXT.md`。竞品和系统通知资料在 `docs/research/`；本机实现、性能测量和验证范围见 `docs/validation.md`。

## Product Principles

- 保存和提醒状态明确可见。
- 待办服务于快速记录和阅读；原野让景观自身成为体验。
- 退出后重开可以继续处理事项；错过提醒不改变完成状态。
- 用户可通过统一菜单和直接子命令进入工具。

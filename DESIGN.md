---
name: windtools
description: 本地待办与可互动字符原野，共用一个终端入口。
colors:
  muted: "#97A6B1"
  accent: "#7BC8D6"
  warning: "#D8BE82"
  error: "#E58C8C"
  selection: "#1E343E"
  meadow-sky: "#101D2F"
  meadow-far: "#2C4554"
  meadow-near: "#244947"
  meadow-ground: "#122D26"
  meadow-leaf: "#DBBC70"
typography:
  title:
    fontFamily: "monospace"
  body:
    fontFamily: "monospace"
  label:
    fontFamily: "monospace"
spacing:
  page-inset: "1 column"
  detail-inset: "2 columns"
  task-rhythm: "2 rows"
  field-rhythm: "3 rows"
components:
  selected-row:
    backgroundColor: "{colors.selection}"
    textColor: "{colors.accent}"
    typography: "{typography.title}"
  active-filter:
    backgroundColor: "{colors.selection}"
    textColor: "{colors.accent}"
    typography: "{typography.title}"
  inactive-filter:
    textColor: "{colors.muted}"
    typography: "{typography.label}"
  field-focused:
    typography: "{typography.body}"
  overlay:
    typography: "{typography.body}"
  reminder-warning:
    textColor: "{colors.warning}"
    typography: "{typography.label}"
  status-error:
    textColor: "{colors.error}"
    typography: "{typography.label}"
  meadow:
    backgroundColor: "{colors.meadow-sky}"
---

# Design System: windtools

## Overview

**Creative North Star: "选择一件事，或看一阵风。"**

windtools 的视觉材料是终端单元格、普通等宽字体和键盘反馈。待办使用紧凑的文字层级，让事项、提醒和保存结果容易阅读；原野用整幅天空、山脊、草海和飘叶表达风的变化。统一菜单连接这两种已确认的体验。

待办继承用户的终端背景与正文前景。原野在景观区域显式绘制分层背景。两者共用青色焦点和清楚的文字状态，但景观装饰只属于原野。所有层级都通过字重、留白、选中标记或字符形状表达，不依赖专用字体。

**Key Characteristics:**

- 终端单元格是布局与绘制单位。
- 待办安静、文字优先，原野完整、可互动。
- 颜色与可读状态文字共同表达反馈。
- Unicode、ASCII 和单色模式共享任务结构。

依据：`src/ui.rs`、`src/ui/details.rs`、`src/ui/overlay.rs`、`src/meadow.rs`；方向来自 `PRODUCT.md` 与 `docs/surface-brief.md`。`.impeccable/review/` 中 PNG 和 JSON 是 Ratatui 单元格 QA 渲染，不能证明特定终端模拟器的字体或主题表现。

## Colors

待办的灰蓝、青色和琥珀服务于阅读与状态；原野的深蓝、青绿与金色构成景深。前置令牌记录代码中真实的 RGB 常量。

### Primary

- **青色焦点**（`accent`）：菜单选项、当前筛选、选中事项、详情与表单标题；选中行同时使用加粗和背景。
- **深青选中底**（`selection`）：当前菜单行、事项行和筛选文本的背景，不用于整页铺底。

### Secondary

- **琥珀提醒**（`warning`）：已错过或失败的提醒行、详情提醒、日期解释及确认操作提示。
- **柔红错误**（`error`）：失败状态和完整原因入口，与可执行的错误文案一起出现。

### Tertiary

- **夜蓝天空**（`meadow-sky`）、**灰蓝远山**（`meadow-far`）、**青绿近山**（`meadow-near`）、**深绿地面**（`meadow-ground`）：仅用于原野的四层背景和山脊。
- **金色飘叶**（`meadow-leaf`）：少量移动的叶点。草色由深度计算，公式留在 sidecar；它不是额外的固定颜色令牌。

### Neutral

- **灰蓝辅助文字**（`muted`）：操作说明、未选筛选、完成事项、普通提醒状态、计数和菜单说明。
- **终端默认前景与背景**：使用 Ratatui `Color::Reset`，颜色由用户的终端主题决定。QA 捕获为 Reset 指定的显示替代色不是应用令牌。

**The Redundant State Rule.** 选中、完成、提醒和错误必须保留文字或字符标记；单色模式不能丢失状态含义。

ANSI 模式使用终端 Gray/Cyan/Yellow/Red/Blue 等槽位，不承诺固定 RGB；单色模式将颜色重置，选中项改用反显。ASCII 模式替换景观字符和操作标签，不强制改配色。

## Typography

整个应用使用用户终端设置的等宽字体，代码不选择字体家族、字号、行高或字距。前置 `monospace` 表示平台约束，不是字体安装要求。

- **Title**：同一单元格尺度，Ratatui `BOLD`；用于菜单名、待办页名、详情、表单和对话框标题。部分标题同时使用青色。
- **Body**：终端默认修饰；用于事项、备注、输入内容和对话框正文。
- **Label**：同一尺度，主要使用辅助色；用于操作、计数、提醒和状态。代码没有独立小字号。

**The Terminal Type Rule.** 用真实的终端加粗、字符标记和空行形成层级，不引入网页字号、展示字体或图标字体。

中文和粘贴文本按 grapheme 与显示宽度处理；截断、输入视口及详情换行必须按终端列宽计算。ASCII 操作标签使用英文，用户输入内容保留原文。

## Layout

菜单是居中的文字列，宽度为窗口减去 8 列、上限 76 列，顶部留 2 行。待办页左右各留 1 列，标题位于第 0 行，筛选位于第 2 行，列表从第 4 行开始；每件事项占 2 行，标题下方缩进显示提醒。

窗口宽度达到 100 列时，待办主体按 64% 清单 / 36% 详情分配。小于 100 列保留单列清单，通过独立详情页阅读完整内容。搜索占用主体顶部 2 行，输入视口保持光标与尾部可见。

表单居中，宽度为窗口减去 6 列、上限 90 列。字段以 3 行为节奏，包含标题、输入行和底部字符边线。一般页面至少需要 40×14 单元格；表单至少 40×20。尺寸不足时显示实际尺寸、所需尺寸和退出操作。

对话框居中，宽度为窗口减去 4 列、上限 80 列；帮助和完整错误高度上限 24 行，其他对话框上限 15 行，且始终不超过窗口减去 2 行。正文可滚动，底部操作保持独立。详情页同样支持按行与按页滚动。

常规页底部保留操作行、空行和状态行。操作文案按宽度收紧；原野将景观铺满除底部 3 行外的区域，这 3 行分别给会话提醒状态、风向/暂停/种子和键盘控制。

## Elevation & Depth

待办页面平坦，无阴影。选中底色体现状态；对话框通过清除局部单元格、绘制字符边框并覆盖原页面表达前后关系。原野通过山脊重叠、草的深度色与高度、叶点位置表达空间，镜头固定。

## Shapes

表单使用单一底边；对话框使用 Ratatui 默认字符边框。事项以 `>`、`[ ]` 和 `[x]` 表达选中与完成状态。没有像素圆角、胶囊或网页卡片形状。

原野使用 `▄` 山脊、`│` / `╱` / `╲` 草叶和 `·` 飘叶；ASCII 对应 `_`、`|` / `/` / `\` 和 `'`。单色山体额外使用稀疏 `.` 纹理，保留山层可见性。

## Components

### Navigation and selected rows

菜单选项、事项行和当前筛选复用青色文字、深青背景和终端加粗。事项仍保留 `>` 与完成框；菜单行保留数字入口。非选中筛选使用辅助色。交互是键盘移动与激活，代码没有鼠标悬停样式。

### Fields and search

字段正文继承终端前景；焦点只改变底部边框为青色，非焦点边框使用辅助色，当前字段显示真实终端光标。搜索使用 `/ ` 前缀、青色文字与横向视口。

### Reminder and status

提醒在事项下方独占一行。已错过和发送失败使用琥珀并保留状态文字。页脚反映保存与会话提醒状态；错误使用红色，完整错误在可滚动对话框中可读。

### Overlay

帮助、确认、备份路径、恢复预览和歧义时间选择共用字符边框容器。青色标题与边框、普通正文、琥珀操作说明形成稳定层级；路径输入复用字段。

### Meadow

山形、草位置和叶点由种子及窗口尺寸生成；重新调整尺寸时重新生成布局。草随时间与风向弯曲，阵风在时间中衰减，叶点沿风向穿过场景。默认绘制目标为 30 FPS，暂停冻结场景时间并停止动画刷新。草数量上限 1,800，叶点数量按宽度计算并限制为 3–40。

## Do's and Don'ts

### Do:

- Do 用终端列宽与 grapheme 处理输入、截断和换行。
- Do 保留完成框、选中标记、暂停文字和完整提醒状态。
- Do 在窄窗口保留主任务，通过滚动或独立详情阅读完整内容。
- Do 让表单焦点、保存失败和会话提醒状态可见。
- Do 将原野的角色形状同步维护 Unicode、ASCII 和单色版本。

### Don't:

- Don't 把 QA 捕获的终端默认颜色替代值写成应用的前景或背景令牌。
- Don't 把景观装饰带入事务清单。
- Don't 用专用字体、网页字号或图标字体替代终端字符层级。
- Don't 让状态只靠颜色区分。
- Don't 将单元格 QA 渲染声明为真实终端模拟器或跨平台验收证据。

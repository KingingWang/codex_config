# Codex 配置助手

一个用 Rust（egui / eframe）写的桌面 GUI，用来图形化编辑 Codex 命令行工具的配置文件：
`~/.codex/config.toml` 以及 `model_catalog_json` 指向的模型目录 JSON。

目标用户是「不想手写 TOML 的新手」：每个设置项都带中文说明和真实字段名，
能下拉选择的绝不让你手填，保存前自动备份、自动做配置健康检查。

## 界面预览

| 基础设置 | 模型管理 |
| --- | --- |
| ![overview](screenshots/page-overview.png) | ![models](screenshots/interaction-model-selected.png) |

| 服务商（三种协议） | 添加服务商模板 |
| --- | --- |
| ![providers](screenshots/interaction-provider-added.png) | ![new provider](screenshots/dialog-new-provider.png) |

| 高级选项 | 保存前差异对比 |
| --- | --- |
| ![advanced](screenshots/page-advanced.png) | ![diff](screenshots/dialog-diff.png) |

更多截图见 [`screenshots/`](screenshots/)（由 GUI 测试自动渲染生成）。

## 功能

- **基础设置**：当前模型 / 当前服务商 / 推理强度 / 思考摘要 / 详细程度 / 人格 / 配置档，
  沙箱模式与审批策略，`model_catalog_json` 的指向管理，以及常用输出开关。
- **模型管理**：对模型目录 JSON 做增删改查。列表带搜索、服务商徽章、上下文大小、推理档位；
  编辑器按「基本信息 / 推理与思考 / 上下文与截断 / 能力与工具 / 高级」分组，
  每个字段都有中文解释；支持「复制为新模型」「从服务商拉取模型列表一键导入」。
- **服务商**：`[model_providers.*]` 的完整编辑。三种协议（Responses / Chat Completions /
  Anthropic）用大卡片三选一，并说明各自请求的地址；认证方式四选一
  （环境变量 / 直接写 Token / Codex 登录态 / 不需要）；支持自定义请求头、查询参数、重试与超时；
  内置 10 个一键模板（OpenAI 官方、Anthropic、中转站、Ollama、LM Studio、DeepSeek、Moonshot、
  Gemini、Azure 等）；可以**真实发一条测试请求**验证地址/协议/密钥是否正确。
- **模型绑定服务商**：每个模型都可以单独选择走哪个服务商，选中模型时 Codex 会自动切换。
- **配置档**：`[profiles.*]` 的增删改与一键启用（`profile = "名字"`）。
- **高级选项**：`[features]` 实验开关（默认/开/关 三态）、`[tui]` 主题与状态栏、
  `[agents]`/`[memories]`/`[notice]`、自定义系统说明、MCP 服务器增删改。
- **源文件编辑**：直接改原始 TOML / JSON，实时校验格式，应用后其它页面立即同步。
- **保存前差异对比**：红绿 diff 展示磁盘旧内容与即将写入的新内容。
- **安全**：只在点「保存」时写盘；写入前自动备份（`config.toml.bak-时间戳`，最多保留 20 份）；
  原子写入（临时文件 + rename）；用 `toml_edit` 做无损编辑，**你的注释和排版都会保留**。
- **健康检查**：模型不在目录里、服务商不存在、地址格式不对、推理档位不匹配等问题
  会在顶栏和「基础设置」里用错误/警告标出，点一下可以跳到对应页面。

## 构建与运行

```bash
cargo run --release          # 开发/运行
cargo build --release        # 产物在 target/release/codex-config
cargo test                   # GUI 测试：渲染每一页并写入 screenshots/
```

启动后默认读取 `$CODEX_HOME`（未设置时为 `~/.codex`）。
也可以在侧栏底部或「基础设置 → 文件位置」里切换到别的配置目录。

## 目录结构

```text
src/
  main.rs            入口（eframe 原生窗口）
  lib.rs             库入口，方便测试驱动 GUI
  app.rs             应用外壳：导航、保存/放弃、差异窗口、toast
  dialogs.rs         所有弹窗：添加/重命名/删除 模型、服务商、配置档
  editors.rs         模型 / 服务商 / 配置档 的编辑缓冲区
  net.rs             后台 HTTP：测试连接、拉取模型列表
  page.rs            页面枚举
  doc/               配置读写层（与 UI 无关，可单测）
    mod.rs           Document：载入 / 保存 / 备份 / 原子写
    toml_ext.rs      基于路径的 toml_edit 无损读写
    catalog.rs       模型目录 JSON 的读写与模板
    providers.rs     [model_providers.*] 读写
    schema.rs        所有枚举取值 + 中文说明 + 服务商模板
    validate.rs      配置健康检查规则
  ui/
    theme.rs         配色、字体（自动加载中文字体）、控件默认样式
    widgets.rs       卡片 / 字段行 / 下拉 / 三态开关 / 弹窗 等组件
    pages/           六个页面
tests/gui.rs         egui_kittest GUI 测试（截图 + 模拟点击/输入）
```

## 设计说明

- **为什么是 egui**：纯 Rust、无 WebView 依赖、单二进制；配合自定义深色主题和
  自动加载的系统中文字体（PingFang / Hiragino Sans GB / 微软雅黑 / Noto CJK），
  在 macOS / Linux / Windows 上都能得到一致的观感。
- **为什么不直接 serde 反序列化整个 config.toml**：Codex 的配置字段非常多且版本变化快，
  整体反序列化会丢掉它不认识的字段。这里用 `toml_edit` 按路径读写、
  模型目录用 `serde_json::Value` 保留未知字段，只改用户碰过的键。
- **GUI 测试**：`tests/gui.rs` 用 `egui_kittest` 离屏渲染整个应用（含真实字体），
  对每一页截图，并模拟侧栏点击、列表选择、文本输入、模板点击等真实交互，
  断言内存中的配置和落盘后的文件内容都正确。

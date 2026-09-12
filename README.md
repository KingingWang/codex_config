# Codex 配置助手

一个用 Rust（egui / eframe）写的桌面 GUI，用来图形化编辑 Codex 命令行工具的配置文件：
`~/.codex/config.toml` 以及 `model_catalog_json` 指向的模型目录 JSON。

本工具专为 fork 版 Codex 项目 **[KingingWang/codex](https://github.com/KingingWang/codex)** 定制：
该分支支持自定义模型目录（`model_catalog_json`）和多服务商，但这些都得手写 TOML/JSON 才能配好，
对新手很不友好。这个 GUI 就是为了让不熟悉命令行的用户也能轻松改配置、加模型、加服务商。

目标用户是「不想手写 TOML 的新手」：每个设置项都带中文说明和真实字段名，
能下拉选择的绝不让你手填，保存前自动备份、自动做配置健康检查。

## 下载 / 安装（推荐）

到 [Releases](https://github.com/KingingWang/codex_config/releases) 页面按你的系统下载，**双击即可运行**，不需要装 Rust 或命令行：

| 系统 | 下载文件 | 使用方法 |
| --- | --- | --- |
| **macOS**（Intel + Apple Silicon 通用）| `codex-config-macos-universal.dmg` | 打开 dmg，把 App 拖进「应用程序」。首次打开若提示未验证开发者，右键点图标选「打开」。 |
| **Windows 10/11 (64 位)** | `codex-config-windows-x64.zip` | 解压后双击 `codex-config.exe`。SmartScreen 提示时点「更多信息 → 仍要运行」。 |
| **Linux (x86_64)** | `codex-config-linux-x86_64.AppImage` | `chmod +x` 后双击运行；依赖已打包在内，跨发行版可用。 |

> 安装包由 GitHub Actions 自动构建，未做正式代码签名/公证，所以首次打开会有上面提到的系统提示，属正常现象。

## 功能

- **新手首页**：暖白与鼠尾草绿的清爽界面，先选模型、服务商和思考深度；权限可一键选择「先看看，不改文件」或「在项目里帮我工作」。回答偏好、文件目录等选项按需展开，专业字段名可悬停查看。
- **基础设置**：当前模型 / 当前服务商 / 推理强度 / 思考摘要 / 详细程度 / 人格 / 配置档，
  沙箱模式与审批策略，`model_catalog_json` 的指向管理，以及常用输出开关。
- **模型管理**：对模型目录 JSON 做增删改查。列表带搜索、服务商徽章、上下文大小、推理档位；
  编辑器按「基本信息 / 推理与思考 / 上下文与截断 / 能力与工具 / 高级」分组，
  每个字段都有中文解释；支持「复制为新模型」「从服务商拉取模型列表一键导入」。
- **服务商**：`[model_providers.*]` 的完整编辑。三种协议（Responses / Chat Completions /
  Anthropic）用大卡片三选一，并说明各自请求的地址；认证方式四选一
  （环境变量 / 直接写 Token / Codex 登录态 / 不需要）；支持自定义请求头、查询参数、重试与超时；
  内置多个一键模板（OpenAI 官方、Anthropic、中转站、Ollama、LM Studio、DeepSeek、Moonshot、
  Gemini、Azure 等）；可以**真实发一条测试请求**验证地址/协议/密钥是否正确。
- **模型绑定服务商**：每个模型都可以单独选择走哪个服务商，选中模型时 Codex 会自动切换。
- **配置档**：`[profiles.*]` 的增删改与一键启用（`profile = "名字"`）。
- **高级选项**：`[features]` 实验开关（默认/开/关 三态）、`[tui]` 主题与状态栏、
  `[agents]`/`[memories]`/`[notice]`、自定义系统说明、MCP 服务器增删改。
- **一键重启 App Server**：Codex 的后台服务只在启动时读一次配置，改完必须重启才生效。
  本工具会扫描正在运行的 app-server（区分 ChatGPT 桌面应用 / Zed / VS Code 等宿主），
  勾选后重启对应进程，宿主会自动拉起读取新配置的新进程；**托管当前会话的实例会被自动保护、不会误杀**。
- **源文件编辑**：直接改原始 TOML / JSON，实时校验格式，应用后其它页面立即同步。
- **保存前差异对比**：红绿 diff 展示磁盘旧内容与即将写入的新内容。
- **安全**：只在点「保存」时写盘；写入前自动备份（`config.toml.bak-时间戳`，最多保留 20 份）；
  原子写入（临时文件 + rename）；用 `toml_edit` 做无损编辑，**你的注释和排版都会保留**。
- **防误操作**：撤销修改、关闭未保存的窗口时确认；源文件草稿须先应用再保存；新建模型目录也等到保存才写入磁盘。已保存与需要重启的状态分开提示。
- **健康检查**：模型不在目录里、服务商不存在、地址格式不对、推理档位不匹配等问题
  会在顶栏和「基础设置」里用错误/警告标出，点一下可以跳到对应页面。

## 从源码构建

需要 Rust 工具链（stable）。

```bash
cargo run --release          # 开发/运行
cargo build --release        # 产物在 target/release/codex-config
cargo test                   # GUI 测试：离屏渲染每一页并模拟交互
```

界面测试使用临时配置目录，不访问真实服务商，也不会修改个人配置。
运行 `cargo test -- --test-threads=1` 会生成 `screenshots/` 下的整页、1000×660 小窗口及操作截图。
覆盖导航、键盘保存、权限预设、折叠设置、配置档覆盖、保存备份、撤销/退出保护、
目录创建、源文件应用、错误配置与删除取消。个人配置冒烟测试默认忽略，需显式启用。

Linux 下也可以实际启动桌面窗口并截图（本机需有 Xvfb、xwininfo、Python Pillow）：

```bash
cargo build --bin codex-config
xvfb-run -a -s "-screen 0 1440x960x24" \
  python3 tools/smoke_desktop.py --binary target/debug/codex-config
```

若设置了 `CARGO_TARGET_DIR`，请将 `--binary` 替换为实际产物路径。
截图写入 `screenshots/native-desktop.png`；脚本只启动测试配置实例，并在结束后关闭该实例。

启动后默认读取 `$CODEX_HOME`（未设置时为 `~/.codex`）。
也可以在侧栏底部或「基础设置」里切换到别的配置目录。

### Linux 构建依赖

egui 需要系统的 OpenGL / X11 / Wayland 开发库，Debian/Ubuntu 上：

```bash
sudo apt-get install -y libgl1-mesa-dev libxkbcommon-dev libwayland-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxcb-icccm4-dev libxcb-keysyms1-dev
```

## 发布流程（维护者）

发布由 [`.github/workflows/release.yml`](.github/workflows/release.yml) 驱动：
推送一个 `v` 开头的标签即可自动为三个平台构建安装包并创建 Release。

```bash
git tag v0.1.0
git push origin v0.1.0
```

也可以在 Actions 页面手动 “Run workflow” 只构建产物、不发 Release，用来试跑。
Linux 采用 AppImage（在 ubuntu-22.04 上构建以兼容更老的 glibc），把依赖打包进单文件，
解决不同发行版跑不起来的问题；macOS 产出 Intel + Apple Silicon 通用二进制的 `.dmg`。

## 目录结构

```text
src/
  main.rs            入口（eframe 原生窗口 + 窗口图标）
  lib.rs             库入口，方便测试驱动 GUI
  app.rs             应用外壳：导航、保存/放弃、差异窗口、toast、重启入口
  dialogs.rs         所有弹窗：添加/重命名/删除 模型、服务商、配置档、重启 App Server
  editors.rs         模型 / 服务商 / 配置档 的编辑缓冲区
  net.rs             后台 HTTP：测试连接、拉取模型列表
  server.rs          扫描 / 归属 / 重启 app-server 进程（只读扫描 + 受保护重启）
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
    icons.rs         Phosphor 图标常量
    widgets.rs       卡片 / 字段行 / 下拉 / 三态开关 / 弹窗 等组件
    pages/           各功能页面
assets/icon/         应用图标（svg / icns / ico / png 多格式）
packaging/           Linux .desktop 等打包资源
build.rs             Windows 下把图标嵌入 exe
.github/workflows/   三平台发布流水线
tests/gui.rs         egui_kittest GUI 测试（离屏渲染 + 模拟点击/输入）
```

## 设计说明

- **为什么是 egui**：纯 Rust、无 WebView 依赖、单二进制；配合自定义浅色主题和
  自动加载的系统中文字体（PingFang / Hiragino Sans GB / 微软雅黑 / Noto CJK），
  在 macOS / Linux / Windows 上都能得到一致的观感。
- **为什么不直接 serde 反序列化整个 config.toml**：Codex 的配置字段非常多且版本变化快，
  整体反序列化会丢掉它不认识的字段。这里用 `toml_edit` 按路径读写、
  模型目录用 `serde_json::Value` 保留未知字段，只改用户碰过的键。
- **重启 app-server 的取舍**：不同宿主（ChatGPT 桌面应用、Zed…）各自拉起一个 app-server，
  且都读同一份配置。本工具只做只读扫描 + 追溯父进程识别宿主，重启即结束旧进程让宿主自动重拉，
  并始终保护托管当前会话的实例，避免中断正在进行的任务。
- **GUI 测试**：`tests/gui.rs` 用 `egui_kittest` 离屏渲染整个应用（含真实字体），
  对每一页截图，并模拟侧栏点击、列表选择、文本输入、模板点击等真实交互，
  断言内存中的配置和落盘后的文件内容都正确。

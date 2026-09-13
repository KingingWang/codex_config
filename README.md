# Codex 配置助手

一个用 Rust（egui / eframe）写的桌面 GUI，用来图形化编辑 Codex 命令行工具的配置文件：
本机或 SSH 远程机器的 `config.toml` 以及 `model_catalog_json` 指向的模型目录 JSON。

本工具专为 fork 版 Codex 项目 **[KingingWang/codex](https://github.com/KingingWang/codex)** 定制：
该分支扩展了 Chat Completions / Anthropic 协议和模型绑定服务商等能力，常需要手写 TOML/JSON 才能配好，
对新手很不友好。这个 GUI 就是为了让不熟悉命令行的用户也能轻松改配置、加模型、加服务商。

目标用户是「不想手写 TOML 的新手」：每个设置项都带中文说明和真实字段名，
能下拉选择的绝不让你手填，保存前自动备份、自动做配置健康检查。

## 下载 / 安装（推荐）

到 [Releases](https://github.com/KingingWang/codex_config/releases) 页面按你的系统下载，**双击即可运行**，不需要装 Rust 或命令行：

| 系统 | 下载文件 | 使用方法 |
| --- | --- | --- |
| **macOS**（Intel + Apple Silicon 通用）| `codex-config-macos-universal.dmg` | 打开 dmg，把 App 拖进「应用程序」。首次打开若提示未验证开发者，右键点图标选「打开」。 |
| **Windows 10/11 (x64，Intel / AMD)** | `codex-config-windows-x64.zip` | 解压后双击 `codex-config.exe`。SmartScreen 提示时点「更多信息 → 仍要运行」。 |
| **Windows 11 (ARM64)** | `codex-config-windows-arm64.zip` | 解压后双击 `codex-config.exe`；适用于 ARM 设备。 |
| **Linux (x86_64)** | `codex-config-linux-x86_64.AppImage` | `chmod +x` 后运行；需要系统提供图形库和 FUSE，详见下方 Linux 依赖说明。 |
| **Linux (aarch64 / ARM64)** | `codex-config-linux-aarch64.AppImage` | `chmod +x` 后运行；需要系统提供图形库和 FUSE，详见下方 Linux 依赖说明。 |

> 安装包由 GitHub Actions 自动构建，未做正式代码签名/公证，所以首次打开会有上面提到的系统提示，属正常现象。

## 功能

- **暗色工作台**：石墨灰分层面板与蓝色强调，统一字体、间距和控件；首页概览当前环境、配置路径与模型数量，模型与权限随窗口宽度自动分栏或堆叠。
- **远程 SSH 环境**：侧栏「切换环境…」支持本机目录与远程机器切换，从 SSH config 的 Host 别名下拉选择；远程读取和保存都在后台执行，明确显示目标机器。
- **基础设置**：当前模型 / 当前服务商 / 推理强度 / 思考摘要 / 详细程度 / 人格 / 配置档，
  沙箱模式与审批策略，`model_catalog_json` 的指向管理，以及常用输出开关。
- **模型管理**：对模型目录 JSON 做增删改查。紧凑列表带搜索、服务商信息、上下文大小、推理档位，长名称可悬停查看；
  编辑器按「基本信息 / 推理与思考 / 上下文与截断 / 能力与工具 / 高级」分组，
  每个字段都有中文解释；选中只打开编辑，详情中明确设为当前模型。支持「复制为新模型」「从服务商拉取模型列表一键导入」。
- **服务商**：`[model_providers.*]` 的完整编辑。三种协议（Responses / Chat Completions /
  Anthropic）用大卡片三选一，并说明各自请求的地址；认证方式四选一
  （环境变量 / 直接写 Token / Codex 登录态 / 不需要）；支持自定义请求头、查询参数、重试与超时；
  内置多个一键模板（OpenAI 官方、Anthropic、中转站、Ollama、LM Studio、DeepSeek、Moonshot、
  Gemini、Azure 等）；可以**真实发一条测试请求**验证地址/协议/密钥是否正确。
- **模型绑定服务商**：每个模型都可以单独选择走哪个服务商，选中模型时 Codex 会自动切换。
- **配置档**：`[profiles.*]` 的增删改与一键启用（`profile = "名字"`）。
- **高级选项**：`[features]` 实验开关（默认/开/关 三态）、`[tui]` 主题与状态栏、
  `[agents]`/`[memories]`/`[notice]`、自定义系统说明、MCP 服务器增删改。
- **一键重启本机 App Server**：Codex 的后台服务只在启动时读一次配置，改完必须重启才生效。
  本工具会扫描正在运行的 app-server（区分 ChatGPT 桌面应用 / Zed / VS Code 等宿主），
  勾选后重启对应进程，宿主会自动拉起读取新配置的新进程；**托管当前会话的实例会被自动保护、不会误杀**。
- **源文件编辑**：直接改原始 TOML / JSON，TOML 语法高亮、JSON 纯文本编辑；实时校验格式，错误区域独立滚动，先应用到界面再保存到文件。
- **保存前差异对比**：红绿 diff 展示磁盘旧内容与即将写入的新内容。
- **安全保存**：只在点「保存」时写盘；先检查文件是否被外部修改，再暂存全部写入并备份。
  每个配置文件分别保留最近 20 份时间戳备份，模型目录在外部或子目录中也适用，不清理其他文件的备份。
  临时文件排他创建；Unix 上新文件和备份权限为 `0600`；单文件原子替换，后续替换失败时尝试回滚。
- **保留原内容**：仅写回实际变化的字段，保留未改字段、注释、未知模型元数据和自定义推理说明。
  内联 TOML 表可正常读取；编辑时必要的内联表转换可能改变局部排版，但不会清空同级配置。
- **防误操作**：撤销修改、关闭未保存的窗口时确认；源文件草稿须先应用再保存；新建模型目录也等到保存才写入磁盘。已保存与需要重启的状态分开提示。
- **编辑保护**：无效数字不会静默删除原值；修正或撤销之前，不能保存或切走丢弃该输入。
  有未保存的模型修改时不能切换模型目录；过期源文件草稿不能覆盖其他页面的新修改。
- **健康检查**：模型不在目录里、服务商不存在、地址格式不对、推理档位不匹配等问题
  会在顶栏和「开始使用」页里用错误/警告标出，点一下可以跳到对应页面。

## 兼容性与安全边界

- 本轮配置约定核对了本地 `KingingWang/codex` 源码版本 `003c77ebdf`。它只内置
  `ollama` / `lmstudio`；OpenAI、Azure 等需要显式添加服务商。模型推理强度支持 `persistent`，
  也保留模型自定义的值。官方 Codex 与该 fork 的配置约定不完全相同，不能直接互换全部设置。
- 健康检查覆盖已知引用关系和结构错误，不是完整的 Codex schema 校验器；
  高级或新版字段仍可通过源文件编辑，未知字段不会被主动删除。
- 网络测试会实际发送一条短请求，可能产生少量费用。测试使用查询参数及环境变量请求头，
  检查协议响应，不把 HTTP 200 的登录网页当成功；为保护密钥，不自动跟随重定向。
  Codex 登录态、命令认证和 AWS 认证暂不支持直接测试，会明确提示，不读取登录文件或执行认证命令。
- 保存检测到外部修改时会保留编辑内容并拒绝覆盖。请先从源文件页复制自己的修改，
  再用「撤销修改」重新载入磁盘版本并合并。符号链接文件暂不支持保存，以免替换链接；
  `config.toml` 为链接时可直接打开目标文件所在的配置目录。
- 多文件保存不是跨文件系统的崩溃原子事务。异常退出、掉电或回滚失败时，请检查提示中的备份；
  恢复前关闭编辑器并复制保留当前文件，再将对应备份复制回原文件。不会自动重启正在使用的 Codex。

## 配置远程 SSH 机器

1. 本机安装 OpenSSH 8.7+ 客户端；已有 SSH config 中需要有具体的 `Host` 别名，例如：

   ```sshconfig
   Host dev-server
       HostName 192.0.2.10
       User developer
       Port 22
       IdentityFile ~/.ssh/id_ed25519
   ```

   上面是示例地址，请使用自己的配置。先在终端确认该别名可通过密钥或 agent 登录，
   并核验、信任主机指纹；本工具不会自动接受陌生指纹。
2. 在侧栏点击「切换环境… → 远程 SSH」，从下拉框选择别名，点击「连接并载入」。
   默认读取本机 `~/.ssh/config`，可展开「SSH 配置文件」指定其他文件并刷新。
3. 远程目录可留空，使用 SSH 非交互会话的 `$CODEX_HOME` 或 `~/.codex`；
   也可填写 `~/custom-codex` 或绝对路径。终端 shell 初始化的变量不一定出现在 SSH 非交互会话中。
   「浏览远程文件夹…」可逐级选择 CODEX_HOME，只更新目录输入，不切换或丢弃当前文档。
   换选别名会清空目录输入，避免把上一台机器的路径带过去。
4. 使用现有模型、服务商、配置档、源文件编辑页面修改，预览后点击「保存配置」写入远端。
   已有模型目录自动读取；更换远程 JSON 文件可在「文件与配置目录 → 选择已有文件」中手填路径或浏览选择。
   新文件通过「创建模型目录」先检查远端路径，再在编辑器中准备，直到保存才会写入。
   在源文件里修改 `model_catalog_json` 时，会先异步读取新的模型目录，成功后一次性应用 TOML 与 JSON；
   新目录不存在、格式不合法、源配置被外部修改或操作被取消时，原配置与草稿保持不变。
   有未保存的模型修改时不能切换路径；删除目录指针不会删除原 JSON 文件。
5. 在「服务商」或「模型管理」点击「拉取模型列表」，由 SSH 远端请求提供商，再勾选导入。
   请求使用编辑器中当前的服务商设置（无需先保存）；导入只修改编辑器，点击「保存配置」才写入远端。
   `base_url` 中的 `localhost` 指远端机器；网络、代理和证书配置也以远端为准。
6. 「服务商 → 检查远端环境变量」检查配置引用的 `env_key` 和 `env_http_headers`，只返回“已设置／缺失”。
   缺失项也显示在健康检查中。结果是 SSH 非交互会话的检查快照，不代表已运行 Codex 进程的环境；
   变量变化后可重新检查，切换目标后清空旧结果。
   「发一条测试请求」同样在远端执行，支持三种协议的短请求，可能产生少量费用；
   取消等待不会撤销已发送的请求或费用，不回传生成正文。

支持范围与边界：

- 远端需 **Linux / macOS 和 Python 3.7+**；助手脚本使用 Python 标准库，通过 SSH 临时执行，不安装常驻服务。
  本地编辑不需要 Python。暂不支持远程 Windows、密码交互、登录态同步或远程重启。
- 复用 SSH config 中的 User、Port、IdentityFile、ProxyJump / ProxyCommand 等连接设置；
  不读取私钥内容，不存储密码，不转发 agent，不打开配置中的端口转发，不执行 LocalCommand。
  SSH 本身仍可能在用户点击连接后执行配置中的 ProxyCommand / Match exec，因此只使用可信配置文件。
- 别名列表静态读取具体 Host 名称（字母、数字、点、下划线、连字符），排序去重；
  支持双引号、多别名、Include 的 `*` / `?` 文件匹配及递归防循环。
  相对 Include 按 OpenSSH 约定以本机 `~/.ssh` 为基准；不展开 Host 通配、否定模式，
  复杂 Include（环境变量、`%`、方括号模式）会提示未展开；Match 条件由 SSH 连接时解释，列表不是连通性保证。
- 读取可取消，连接超时 10 秒，每次 SSH 请求最多 90 秒；读取配置与模型目录最多两次请求。
  操作期间禁用编辑和切换；失败保留原文和草稿，不把加载失败当作空白配置。
- 远程模型拉取支持 Responses / Chat Completions / Anthropic 服务商的模型列表接口，
  使用远端 Python 标准库执行 HTTP 请求，无需额外安装依赖。HTTP 超时 20 秒、响应上限 2 MiB，
  聊天测试复用同样的认证和保护：保留 HTTPS 证书校验，不跟随重定向，不回传原始响应正文或环境变量密钥。
  `env_key` 和 `env_http_headers` 必须在 **SSH 非交互会话** 中可见；缺失时明确报错，
  不退回本机环境变量；助手不会额外加载 `.bashrc` / `.profile` 或执行认证命令。
  配置中直接填写的 Token、自定义请求头和查询参数也受支持；登录态、命令认证、AWS 认证仍不支持。
  若服务商不提供模型列表接口，可继续手动添加准确的模型标识。
- 远程目录浏览只读取元数据，每次最多显示 512 项；可输入准确路径继续浏览或选择文件。
  不跟随目录或条目符号链接，不提供删除、上传或执行文件功能。
  新模型目录的路径预检不会创建文件；保存时仍重新核对文件不存在，防止预检后出现重名文件。
- 保存同时核对 config 和模型目录的原文快照，拒绝覆盖外部修改或符号链接；先暂存、备份，
  再逐个原子替换，第二个文件写入失败时尝试回滚。文件及备份权限为 `0600`，每文件保留最近 20 份备份。
  单文件最大 8 MiB。断线/超时不能保证撤销已完成的远程写入，需重新载入核对，必要时从备份恢复。
- 远程模式不会调用本机文件打开器、本机服务重启或本机服务商网络测试，也不拿本机环境变量检查远端密钥。
  保存后需在远端手动重启 Codex；界面显示的是已载入的快照，不是实时同步或持续连接。

## 从源码构建

需要 Rust 工具链（stable）。

```bash
cargo run --release          # 开发/运行
cargo build --release        # 产物在 target/release/codex-config
cargo test --locked -- --test-threads=1  # 数据层、回环 HTTP 与离屏 GUI 回归
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
```

测试使用临时配置目录、本机回环 HTTP 服务和隔离目录中的远程助手脚本，
不访问真实服务商或 SSH 主机，也不会修改个人配置。Unix 上助手测试需要本机 Python 3。
远程测试覆盖模型拉取和聊天测试的认证、协议、请求参数、重定向与响应限制，
以及环境状态检查、目录浏览与路径预检、源文件原子切换、取消、勾选导入和环境隔离。
运行 `cargo test -- --test-threads=1` 会生成 `screenshots/` 下的整页、1000×660 小窗口及操作截图。
覆盖导航、键盘保存、权限预设、折叠设置、配置档覆盖、保存备份、撤销/退出保护、
目录创建、源文件应用、错误配置与删除取消，也覆盖长中文名称与路径、紧凑导航、模型选中与启用、
键盘配置档操作及源文件错误恢复。个人配置冒烟测试默认忽略，需显式启用。

Linux 下也可以实际启动桌面窗口并截图（本机需有 Xvfb、xwininfo、Python Pillow）：

```bash
cargo build --bin codex-config
xvfb-run -a -s "-screen 0 1440x960x24" \
  python3 tools/smoke_desktop.py --binary target/debug/codex-config
```

若设置了 `CARGO_TARGET_DIR`，请将 `--binary` 替换为实际产物路径。
截图写入 `screenshots/native-desktop.png`；脚本只启动测试配置实例，并在结束后关闭该实例。

启动后默认读取 `$CODEX_HOME`（未设置时为 `~/.codex`）。
也可以在侧栏「切换环境…」选择其他本地目录或远程 SSH 机器。

### Linux 构建依赖

egui 需要系统的 OpenGL / X11 / Wayland 开发库，Debian/Ubuntu 上：

```bash
sudo apt-get install -y libgl1-mesa-dev libxkbcommon-dev libwayland-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxcb-icccm4-dev libxcb-keysyms1-dev
```

离屏 GUI 测试还建议安装 `libegl1 libvulkan1 mesa-vulkan-drivers fonts-noto-cjk`。
AppImage 当前主要提供单文件分发，尚未完整打包这些系统图形依赖；需要 FUSE 的系统还需安装 `libfuse2`。

## 发布流程（维护者）

发布由 [`.github/workflows/release.yml`](.github/workflows/release.yml) 驱动：
推送一个 `v` 开头的标签即可自动为三个平台构建安装包并创建 Release。

```bash
git tag -a v0.5.0 -m "Release v0.5.0"
git push origin v0.5.0
```

也可以在 Actions 页面手动 “Run workflow” 只构建产物、不发 Release，用来试跑。
Linux 采用 AppImage：x86_64 / aarch64 分别在 `ubuntu-22.04` / `ubuntu-22.04-arm` 原生构建，以控制 glibc 基线（2.35）；实际发行版兼容性仍需验证；
macOS 产出 Intel + Apple Silicon 通用二进制的 `.dmg`。

常规 push / PR 的检查见 [`.github/workflows/ci.yml`](.github/workflows/ci.yml)：
Linux 运行格式、Clippy、文档及离屏 GUI 测试并上传截图；Windows / macOS 运行非 GUI 回归测试。
发布工作流另外在 Windows x64 / ARM64、Linux x86_64 / aarch64 上构建并运行非 GUI 回归测试。
ARM64 包仍需实机验证窗口、字体、文件对话框和配置保存；构建成功不代表桌面兼容性已验证。

发布工作流使用原生 ARM runner（`windows-11-arm`、`ubuntu-22.04-arm`）；运行前需确保仓库可使用这些 runner。
可先手动运行 `Release` 工作流：只上传构建产物，不创建 Release；推送 `v*` 标签时才发布全部五个文件。
维护说明见 [CONTRIBUTING.md](CONTRIBUTING.md)，本轮修改见 [CHANGELOG.md](CHANGELOG.md)。

## 目录结构

```text
src/
  main.rs            入口（eframe 原生窗口 + 窗口图标）
  lib.rs             库入口，方便测试驱动 GUI
  app.rs             应用外壳：导航、保存/放弃、差异窗口、toast、重启入口
  dialogs.rs         所有弹窗：添加/重命名/删除 模型、服务商、配置档、重启 App Server
  editors.rs         模型 / 服务商 / 配置档 的编辑缓冲区
  net.rs             后台 HTTP：测试连接、拉取模型列表
  ssh_config.rs      静态读取 Host 别名与 Include 文件
  remote.rs          系统 SSH、后台任务、超时与远程快照协议
  remote_helper.py   远端临时执行的标准库助手：读取、备份、冲突检测、原子替换
  remote_ui.rs       本地/SSH 环境切换、别名下拉与远程操作状态
  remote_browser.rs  只读远程目录浏览、CODEX_HOME 与 JSON 文件选择
  server.rs          扫描 / 归属 / 重启 app-server 进程（只读扫描 + 受保护重启）
  page.rs            页面枚举
  doc/               配置读写层（与 UI 无关，可单测）
    mod.rs           Document：载入 / 保存 / 备份 / 原子写
    persistence.rs   保存冲突检测、暂存、备份保留和失败回滚
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

- **为什么是 egui**：纯 Rust、无 WebView 依赖、单二进制；配合自定义深色主题和
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

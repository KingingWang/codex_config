# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-09-12

### 远程功能补齐

- 通过 SSH 在远端拉取服务商模型列表，回传模型标识供勾选导入；导入后仍需保存才写入远端。
- 在远端执行 Responses / Chat Completions / Anthropic 短聊天测试，不回传生成正文。
- 在远端解析 `env_key` 与 `env_http_headers`，不使用本机同名变量；环境检查只返回是否设置，并接入健康提示。
- 增加只读远程目录浏览，支持选择 CODEX_HOME 和模型目录 JSON；不选择符号链接，目录列表最多 512 项。
- 新建远程模型目录前检查目标路径；保存时再次检测冲突，不覆盖已有文件。
- 源文件修改 `model_catalog_json` 时先加载新目录，再整体应用配置；失败或取消保留原配置和草稿。

### 安全与验证

- 远端服务商请求保留 HTTPS 证书校验，不跟随重定向；HTTP 超时 20 秒，响应最多 2 MiB。
- 统一后台 SSH 任务、取消与环境切换保护；切换环境后清空旧环境变量状态。
- 补充远端请求、环境检查、目录浏览、路径预检和原子切换回归；161 项测试通过，1 项个人配置测试默认跳过。
- 远端仍需 Linux / macOS 和 Python 3.7+；暂不支持远程 Windows、远程重启，以及登录态、命令或 AWS 认证测试。

## [0.3.0] - 2026-09-12

### 界面与远程配置

- 改为炭灰暗色工作台，使用克制的蓝色强调，移除大面积绿色欢迎区。
- 增加本地 / SSH 环境切换，从 SSH config 下拉选择具体 Host 别名，支持 Include。
- 增加后台远程配置与模型目录读取、保存、冲突检测、备份及失败回滚。
- 隔离远程模式的本机文件打开、认证测试与进程重启；取消或加载失败不丢失原有修改。
- 补充 SSH 配置解析、远程助手故障注入和环境切换 GUI 回归测试。

### Added

- Keyboard shortcuts help dialog (press `?` or click "使用帮助")
- Unit tests for `editors.rs` module
- Unit tests for `catalog.rs` module
- CI workflow for running tests and clippy checks on pull requests
- Linux GUI screenshot artifacts and Windows/macOS non-GUI regression jobs.

### Fixed

- Refuse saves that would overwrite externally changed, created, or deleted files.
- Stage all files before publishing; roll back earlier writes on a later commit failure.
- Create private, exclusive temporary files and timestamped backups; retain backups per file.
- Preserve original malformed JSON for repair and backup; protect pending catalog and raw edits.
- Preserve untouched model/provider metadata, explicit defaults, inline TOML siblings and comments.
- Block invalid numeric editor input from being saved or silently discarded.
- Validate profile references, effective model/provider overrides and malformed catalog structures.
- Include provider query parameters and environment headers in probes; fix Anthropic model-list URLs.
- Reject misleading non-JSON successes, response read failures and credential-bearing redirects.
- Report unsupported login/command/AWS authentication without executing commands or reading credentials.
- Align built-in provider IDs and `persistent` effort with the supported Codex fork.

### Changed

- Improved code quality by fixing all clippy warnings
- Refactored `style()` function in `theme.rs` to use struct initialization pattern

## [0.2.0] - 2026-09-12

### Changed

- 重做新手首页、分组导航和配置编辑体验，增加权限预设及进阶设置折叠。

## [0.1.0] - 2026-09-12

### Added

- Initial release of Codex 配置助手
- GUI for editing `~/.codex/config.toml` and model catalog JSON
- Support for multiple model providers (OpenAI, Anthropic, DeepSeek, Moonshot, Gemini, Azure OpenAI, Ollama, LM Studio, custom relays)
- Three wire protocols: Responses, Chat Completions, Anthropic
- Model management with search, filtering, and batch import from providers
- Provider templates for quick setup
- Profile management for preset configurations
- Raw TOML/JSON editing with syntax validation
- Configuration health check with error/warning/info categorization
- App Server restart dialog with host detection (ChatGPT Desktop, Zed, VS Code)
- Automatic backup before save (keeps up to 20 backups)
- Chinese-first UI with tooltips showing raw config field names
- Progressive disclosure for advanced settings
- Cross-platform support: macOS (Universal), Windows x64, Linux AppImage

### Security

- API keys stored in config files (user can choose environment variable mode for better security)
- Atomic file writes to prevent corruption
- Protected restart for current session's host process

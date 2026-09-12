# Contributing to Codex 配置助手

感谢你有兴趣为这个项目做贡献！

## 开发环境设置

1. 安装 Rust 工具链（推荐使用 rustup）：
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. 克隆仓库并进入目录：
   ```bash
   git clone https://github.com/KingingWang/codex_config.git
   cd codex_config
   ```

3. 构建项目：
   ```bash
   cargo build
   ```

4. 运行测试：
   ```bash
   cargo test --locked -- --test-threads=1
   ```

## 开发流程

### 分支命名

- `feature/xxx` - 新功能
- `fix/xxx` - Bug 修复
- `docs/xxx` - 文档更新
- `refactor/xxx` - 代码重构

### 提交信息格式

使用 Conventional Commits 格式：

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

类型包括：

- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式调整（不影响功能）
- `refactor`: 重构
- `test`: 测试相关
- `chore`: 构建/工具链相关

### 代码质量要求

提交前请确保：

1. **编译通过**：
   ```bash
   cargo build --all-targets
   ```

2. **测试通过**：
   ```bash
   cargo test --locked -- --test-threads=1
   ```

3. **无 Clippy 警告**：
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```

4. **格式正确**：
   ```bash
   cargo fmt --check
   ```

### GUI 测试

GUI 测试使用 `egui_kittest` 进行离屏渲染：

```bash
# 运行所有 GUI 测试并生成截图
cargo test -- --test-threads=1

# 截图会保存在 screenshots/ 目录
```

### 个人配置冒烟测试

测试你自己的 Codex 配置（不会修改文件）：

```bash
cargo test --test gui loads_a_real_codex_home_if_present -- --ignored --nocapture
```

## 项目结构

```
src/
  main.rs            # 入口
  lib.rs             # 库入口
  app.rs             # 应用外壳：导航、保存、弹窗
  dialogs.rs         # 所有弹窗
  editors.rs         # 编辑器缓冲区
  net.rs             # HTTP 请求
  server.rs          # App Server 扫描/重启
  page.rs            # 页面枚举
  doc/               # 配置读写层
    mod.rs           # Document 结构
    persistence.rs   # 冲突检测、暂存、备份和回滚
    toml_ext.rs      # TOML 路径访问
    catalog.rs       # 模型目录 JSON
    providers.rs     # 服务商读写
    schema.rs        # 枚举定义和模板
    validate.rs      # 健康检查
  ui/
    theme.rs         # 主题和样式
    icons.rs         # 图标常量
    widgets.rs       # UI 组件
    pages/           # 各页面实现
tests/
  gui.rs             # GUI 集成测试
  document.rs        # 文档操作测试
```

## 添加新的配置字段

1. 在 `src/doc/schema.rs` 添加选项定义
2. 在 `src/editors.rs` 添加编辑器字段
3. 在 `src/ui/pages/` 相关页面添加 UI 控件
4. 在 `src/doc/validate.rs` 添加验证规则（如需要）
5. 更新测试

先用临时目录或回环 HTTP 服务复现问题，再修改实现。不要让普通测试读取个人配置、
执行认证命令、联系真实模型服务或重启用户进程。修改任一字段时，要测试未知字段、
注释、显式 `false` 和自定义元数据没有被顺带改写。

Linux 的构建和离屏渲染依赖见 [README](README.md#linux-构建依赖)。

## 本地化

当前项目使用中文作为主要语言。如需添加其他语言支持，需要：

1. 创建本地化字符串表
2. 修改所有 UI 文本使用本地化键
3. 添加语言切换功能

## 发布流程

维护者专属。推送 `v` 开头的标签即可触发自动发布：

```bash
git tag -a v0.4.0 -m "Release v0.4.0"
git push origin v0.4.0
```

## 行为准则

- 尊重所有贡献者
- 接受建设性批评
- 关注什么对项目最有利
- 对社区保持友善和包容

## 许可证

本项目采用 MIT 许可证。贡献的代码将以相同许可证发布。

# macOS CI 修复：开发示例的平台边界

> 历史实现与验收记录：本文保留当时的平台矩阵和测试数字。现行范围为 DEB、RPM、AppImage、macOS arm64，入口见 [RELEASE_TARGETS.md](RELEASE_TARGETS.md)；后续风险处置见 [OPTIMIZATION_ITERATION.md](OPTIMIZATION_ITERATION.md)。

> 历史修复记录。0.3.2 起的现行发布目标为 DEB、RPM、AppImage、macOS arm64，参见 [RELEASE_TARGETS.md](RELEASE_TARGETS.md)。

## 失败定位

基线提交：`1b828d5d47525cdb8832023f8e5aa37a32907f99`。用户截图与 GitHub run `34124343773` 指向同一处失败：`cargo test --workspace --locked` 编译 `liusheng-core` 的 `examples/dev.rs` 时，`alsa_sink` 和 `pipewire_sink` 两处无条件导入触发 E0432。

`audio/mod.rs` 中 Linux 模块的 `#[cfg(target_os = "linux")]` 已正确隔离平台。Cargo 默认测试范围会编译 examples，应用的主二进制和核心库编译成功仍需保证开发示例的平台边界完整。

本地在修改前执行真实目标检查：

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo check -p liusheng-core --all-targets --locked --target aarch64-apple-darwin
```

结果为退出码 101，出现与截图一致的两条 E0432。日志保存在 `target/qa/macos-ci-fix/apple-before.log`。

## 修复内容

### 开发示例

`dev play` 通过目标平台选择 `NativeSink`：Linux 使用 PipeWire，macOS 使用 CoreAudio。ALSA 独占探测、硬件音量探测及相关导入/函数仅为 Linux 编译；macOS 上调用这些命令时返回清晰的平台说明。帮助文本显示当前平台的输出后端。

扫描默认目录复用 `AppSettings`，macOS 使用用户音乐目录。扫描和搜索使用各自的临时数据库，适合并行测试；输入缺失与未知命令返回常规错误。解码默认输出使用系统临时目录。

`Cargo.toml` 显式将示例设置为 `test = true`，默认 workspace 测试会运行示例单元测试。共同用例覆盖帮助、参数验证、目录选择、精确解码与数据库隔离；macOS 额外覆盖 Linux 探测命令的错误语义。

`scripts/check-dev-cli.py` 直接运行构建出的开发工具，在隔离目录生成 16/24 位 WAV 并验证解码样本、扫描和搜索结果。该测试在 Linux 执行 8 个检查，macOS 原生执行时包含两个额外的平台限制检查。整个流程保持音频设备关闭。

### 目录监听路径一致性

对锁定的 notify 8.2.0 FSEvents 源码进行检查后，补充了后端路径与曲库键值的映射边界。监听根目录注册时记录规范路径与用户配置路径，事件回调将规范路径映射回配置路径。

这样，`/private/var/...` 与 `/var/...` 等路径别名以及符号链接根目录可保持曲库索引的一致性。删除事件仅做路径组件映射，已删除文件也能定位原索引。存在多个配置别名时分别回传对应路径，原有防抖负责去重。

新增用例覆盖规范路径、删除事件、路径组件边界、多个别名，以及真实符号链接目录下的文件创建/删除事件。曲库数据库结构、图标、界面和音频引擎协议继续沿用现有实现。

### CI 与发布

普通检查工作流保留 Linux 原有完整回归，并增加真实 Apple Silicon / Intel 目标的核心库 `--all-targets` 检查。

原生 macOS job 拆为 `macos (arm64)` 和 `macos (x86_64)`：

1. 在 Qt 桌面构建前检查核心库、所有示例和测试的编译，并确认宿主架构。
2. 安装 native Qt 后运行完整 workspace 严格 Clippy、单元/集成/文档测试。
3. 构建并执行开发工具的硬件无关命令检查。
4. 构建实际应用并通过 `check-desktop-smoke.py` 加载初始 QML 场景、正常退出。
5. 无论结果如何，上传各架构的诊断日志。

发布工作流也在 macOS 打包前执行 workspace 测试与开发工具检查。`package-macos.sh` 向 Cargo 显式传递将要打包的 `--target-dir`，保持编译与取用产物的目录一致。

`tests/test_macos_package.py` 使用明确的外部工具替身，在临时目录验证默认输出目录、带空格的相对输出目录、构建失败时保留旧文件且停止打包，以及版本不匹配时提前退出。该测试覆盖脚本编排，原生编译、部署和签名由 macOS runner 验证。

## 验证口径

| 检查 | 本地验证范围 |
| --- | --- |
| aarch64-apple-darwin / x86_64-apple-darwin | 真实 Apple 目标的核心库、示例、测试 metadata 编译与严格 Clippy |
| Linux workspace | 105 个 Rust 测试，包括 CoreAudio 通用接口用例、5 个开发示例测试与新增路径映射用例 |
| 开发工具 | 原生 Linux 可执行文件的 8 项 CLI 检查，含 16/24 位样本精确比对 |
| Python 回归 | 图标、安装和 macOS 打包脚本编排共 16 项 |
| QML 控件 | 23 个用例；框架初始化与清理另计两项 |
| GitHub Actions 配置 | actionlint 1.7.12 检查两个 workflow |
| 应用回归 | release 构建、QML 初始加载、隔离页面/功能、Wayland 三档缩放与托盘协议检查 |

Apple 交叉目标检查验证 Rust 平台分支和类型约束。完整 macOS Qt/C++ 链接、Apple 原生测试执行、系统音频设备和实际桌面表现由原生 macOS 环境验证。当前开发容器运行 Linux，远程工作流需由修复提交触发；只有新运行完成后才能确认两个原生 job 的最终状态。

主要日志位于 `target/qa/macos-ci-fix/`；最终应用构建与回归位于其 `final/` 子目录。

## 重复执行

```sh
# 当前平台：完整测试以及原生开发工具
cargo test --workspace --locked
just dev-cli-test

# Linux 上提前检查两个 Apple 目标的平台边界
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo check -p liusheng-core --all-targets --locked --target aarch64-apple-darwin
cargo check -p liusheng-core --all-targets --locked --target x86_64-apple-darwin

# 原生 macOS，安装 Qt 并将其 bin 加入 PATH 后
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build -p liusheng-core --example dev --locked
python3 scripts/check-dev-cli.py target/debug/examples/dev
cargo build -p liusheng --locked
python3 scripts/check-desktop-smoke.py target/debug/liusheng
```

参考：Cargo `cargo-test` 的 Target Selection 章节、Rust Reference 的 Conditional compilation 章节、GitHub-hosted runners 参考，以及项目锁定的 `notify-8.2.0/src/fsevent.rs`。


## 后续原生事件契约修复

`e4c3c88` 的 macOS arm64 原生测试出现“目录 + 文件”通知与单文件断言不一致。该问题的等价重放、监听器防抖加固及测试分层见 [WATCHER_CONTRACT_FIX.md](WATCHER_CONTRACT_FIX.md)。原生测试允许系统送达形态的差异，同时要求曲库完成实际更新。

# macOS 打包路径与符号链接回归修复

基线：`b510cdc` / `v0.3.6`。本轮修复测试和发布验证链路，播放器源码与版本号保持原样。新标签应在修复提交的原生 macOS CI 通过后创建。

## 根因

截图中失败的是 `tests/test_macos_package.py` 的两个路径断言：

- `test_default_target_is_explicit`
- `test_relative_custom_target_with_spaces`

测试夹具从 Python `TemporaryDirectory` 取得 `/var/folders/...` 形式的路径，直接把它转换成字符串作为期望值。打包脚本使用 `pwd -P` 确定项目真实目录，子进程工作目录也会解析符号链接，因此实际 Cargo 参数可以包含 `/private/var/folders/...`。

这两种写法在该 runner 上对应同一目录。原断言比较字符串，导致正确的构建目录被判定为错误。普通 CI 和标签 Release 复用 `quality.yml`，因此同一断言同时阻止两条流程。

上一轮可移植验证覆盖了 Cocoa 插件选择，但没有主动构造带符号链接的临时目录。这次补齐了文件系统语义测试。

## 可复现证据

在 Linux 容器中创建私有的实体临时目录及指向它的符号链接，把 `TMPDIR`、`TMP`、`TEMP` 设置为不同拼写，分别运行原来的整套 Python 测试。系统 `/var` 和 `/private` 保持原状。

基线结果：

- 实体目录：118 项通过。
- 符号链接目录：9 项失败、2 项异常，包含截图中的两项。
- 扩展检查发现 AppImage 测试的路径字符串假设，以及安装后检查器对合法前缀别名的误判。

日志：`target/qa/macos-path-fix/before-physical.log` 与 `before-aliased.log`。

## 修复内容

### macOS 打包测试

检查 `--target-dir` 和 `--manifest-path` 各出现一次、采用绝对路径、构建完成后存在，并通过 `Path.samefile()` 验证真实文件或目录身份。继续核对 `--locked`、`--release`，以及 ZIP 中的可执行文件字节与指定构建目录中产物一致。

新增符号链接 checkout、相对构建目录、绝对缓存目录、含空格 TMPDIR、独立调用工作目录、符号链接输出目录和 `--no-build` 的测试。错误目录、相对参数、缺失目录继续触发失败；具有相同文件名的其他构建目录也会被拒绝。

打包脚本 `package-macos.sh` 的物理路径策略继续保留。

### Linux 安装与 AppImage 路径契约

AppImage 测试对已有资源比较文件身份，对尚未创建的构建目录比较解析后的绝对路径。相对歌曲参数和用户显式环境覆盖仍按原值检查。

安装后验证器继续要求安装器生成的带引号 `Exec` 语法与单个 `%U` 参数，同时将启动器路径解析为真实文件后比较。合法目录别名可以通过；指向其他安装位置、额外命令参数和外部图标链接继续失败。

### Cocoa 安装包检查

`smoke_environment()` 在检查资源归属时统一使用真实 `Contents` 路径。包内的合法资源链接可以通过，指向包外的资源继续拒绝。

新增 `scripts/check-macos-bundle.sh`，由普通 CI 和发布流程共用：真实解包、检查 Cocoa 与 `qt.conf`、验证签名、确认 arm64、通过 Cocoa 启动实际 QML。失败会保留诊断，临时解包目录在退出时清理，原 ZIP 保持原样。

普通 `main` / pull request CI 增加实际 macOS 包构建与迁移启动步骤。标签发布的实际包继续由发布任务构建并调用相同验证器，避免在标签的 quality 阶段重复构建发布包。

## 持续回归

`scripts/check-path-contracts.py` 创建私有的 `var -> private/var` 目录结构，在实体路径与符号链接路径两种环境中分别运行检查：

```bash
python3 scripts/check-path-contracts.py --scope linux --output target/qa/path-contracts
python3 scripts/check-path-contracts.py --scope macos --output target/qa/macos/path-contracts
```

Linux 范围执行全部 Python 测试；macOS 范围在安装 Qt 前执行打包和 Cocoa 检查器的可移植测试。检查器要求实际执行测试且全部通过，跳过、零测试、超时或异常均产生失败退出码与 JSON 记录。

本轮增加后共 137 项 Python 测试；两种临时目录环境均通过。其中 macOS 打包 17 项、桌面启动契约 20 项，已分别在两种路径环境执行。

这些可移植测试使用明确的外部工具替身，验证文件、参数、退出码和进程编排。真实 Cocoa 加载、Mach-O 链接与签名的最终结果由原生 macOS CI 提供。本轮 Linux 容器的验证范围单独记录，避免将替身测试计为原生 macOS 验收。

## 发布步骤

保留现有 `v0.3.5`、`v0.3.6` 标签。先推送修复到 `main`，检查 `quality / macos (arm64)` 中的路径矩阵、原生应用及实际安装包验证。全部通过后，再准备下一正式版本与对应标签。

## 参考

- Python `Path.samefile()` / `Path.resolve()`：https://docs.python.org/3/library/pathlib.html
- Bash `pwd -P`：https://www.gnu.org/software/bash/manual/html_node/Bourne-Shell-Builtins.html

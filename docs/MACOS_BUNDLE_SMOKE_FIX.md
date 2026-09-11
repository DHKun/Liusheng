# macOS 安装包启动检查修复

## 故障与根因

`v0.3.5` 的发布任务 `34575621465` 完成了质量检查、macOS arm64 编译、应用打包和签名校验，在 `Verify relocated packaged macOS application` 阶段失败：

```text
Could not find the Qt platform plugin "offscreen"
Available platform plugins are: cocoa.
```

原 `scripts/check-desktop-smoke.py` 为 Linux 和 macOS 一律设置 `QT_QPA_PLATFORM=offscreen`。Qt 开发环境提供离屏测试插件；`macdeployqt` 为可分发应用部署 macOS 的 Cocoa 插件。启动检查使用的后端与最终安装包的运行后端存在差异。

通过仅含 Cocoa 插件的临时 `.app` 和可执行测试替身复现了原检查失败，记录位于 `target/qa/macos-cocoa-fix/reproduction.log`。这是检查脚本行为的可移植复现；原生 Cocoa 加载由 macOS CI 验证。

## 修复

### 平台和环境

- `check-desktop-smoke.py` 默认在 macOS 使用 `cocoa`，Linux 使用 `offscreen`。普通 macOS CI 和发布后的启动检查显式传入 `--platform cocoa`。
- `.app` 检查要求 Cocoa；检查前验证 `Contents/PlugIns/platforms/libqcocoa.dylib` 与 `Contents/Resources/qt.conf` 存在、非空且实际指向包内资源。
- 使用隔离的工作目录、HOME 和 XDG 目录，清除继承的 Qt 插件、QML、主题与渲染覆盖变量。macOS 安装包检查额外清除 `DYLD_*` 覆盖变量，让包内相对配置负责资源定位。
- 保持被测包及签名字节原样。Linux 的动态库路径继续保留，AppImage 由自身启动器配置运行依赖。
- 使用 `--smoke-test --no-update-check --no-online-metadata`，验证初始 QML 场景和正常退出。软件渲染保持启动检查与物理 GPU 性能验收的职责分离。
- 完整保留 25 秒超时、失败退出码及 QML/动态库/插件诊断检查；超时结束对应进程组。

### 打包与诊断

- `package-macos.sh` 在 `macdeployqt` 完成后、签名和 ZIP 归档前检查 Cocoa 插件与 `qt.conf`。
- 发布继续验证解包后的签名、arm64 架构和真实程序启动。插件清单、相对路径配置和签名检查记录在 `packaged-bundle.log`。
- `--debug-plugins` 将 Qt 插件加载信息写入 `packaged-desktop.log`。
- 成功和失败均归档 `macos-release-validation`。诊断附件使用独立名称，与安装包的 `liusheng-*` 下载范围分开。
- Linux 与 macOS 的共用质量工作流都执行检查脚本和打包合同测试。

## 验收

可移植测试覆盖 Cocoa-only 包、普通 macOS 可执行文件、Linux 离屏后端、环境污染、缺失/空插件、包外符号链接、缺失 `qt.conf`、带空格目录、明确失败、零退出码伴随 QML 错误、超时结束与发布门禁保留。

```sh
python3 -m unittest discover -s tests -p 'test_desktop_smoke.py' -v
python3 -m unittest discover -s tests -p 'test_macos_package.py' -v
python3 scripts/check-desktop-smoke.py target/release/liusheng
```

macOS 原生安装包检查：

```sh
python3 scripts/check-desktop-smoke.py /path/to/Liusheng.app/Contents/MacOS/Liusheng \
  --platform cocoa --debug-plugins --output target/qa/macos/packaged-desktop.log
```

本轮运行日志及构建结果保存在 `target/qa/macos-cocoa-fix/`。Debian 开发容器负责可移植测试和 Linux 程序回归；真实 Cocoa、签名和 macOS ZIP 启动以原生 CI 的结果为准。

## 重新发布

修复版本为 `0.3.6`，包含原计划在 `0.3.5` 发布的全部功能。两个 crate、锁文件与 `docs/releases/v0.3.6.md` 已对齐。

`v0.3.5` 标签继续指向原提交。新的 `v0.3.6` 标签在修复提交上触发完整质量检查和 DEB、RPM、AppImage、macOS arm64 发布流程。旧任务的重新运行继续采用其原提交与工作流。

## 参考

- Qt macOS 部署：https://doc.qt.io/qt-6/macos-deployment.html
- Qt 平台选择与命令行参数：https://doc.qt.io/qt-6/qguiapplication.html
- Qt 6.8 macdeployqt 相对路径配置：https://github.com/qt/qtbase/blob/6.8/src/tools/macdeployqt/shared/shared.cpp
- GitHub 重新运行工作流的提交语义：https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs

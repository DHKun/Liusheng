# 四目标发布与 AppImage

## 发布范围

| 产物 | 架构 | 官方构建 / 运行基线 |
| --- | --- | --- |
| DEB | Linux x86_64 | Debian 13 |
| RPM | Linux x86_64 | Fedora 44 |
| AppImage | Linux x86_64 | Debian 13 构建，glibc 2.41+ 桌面 |
| macOS ZIP | arm64 | Apple Silicon，MACOSX_DEPLOYMENT_TARGET=13.0 |

自动构建与发布采用这四种产物，附带 `SHA256SUMS`。Intel macOS job、Intel Apple 交叉检查和 Arch 自动打包工作流已移除。已有标签、已发布的安装包和历史故障记录保留。手工 Arch 工具留在仓库作为历史辅助工具，正式 CI 和 Release 均使用上述范围。

版本以两个 crate 的 `[package].version` 与 workspace Cargo.lock 为准。四目标策略自 0.3.2 开始使用；后文历史验收记录保留当时版本。命令示例中的 `X.Y.Z` 替换为待构建或已下载的版本。

## AppImage 构建

```sh
# 官方构建环境：Debian 13、Python 3.11+、Qt 6.8+
just package-appimage
# 已完成同版本 release 构建时复用产物
python3 scripts/package-appimage.py --no-build
# 指定版本和输出目录；版本须与 crate 一致
python3 scripts/package-appimage.py --version X.Y.Z --output dist
```

完整系统依赖清单在 `.github/workflows/package-appimage.yml`。打包器支持自定义 `CARGO_TARGET_DIR`，构建和安装显式传递同一绝对目录；`CARGO_BUILD_TARGET` 应保持未设置，以使用原生 Linux x86_64 输出目录。

构建流程：

1. 校验宿主架构、应用版本和 ELF 架构，完成 release 构建。
2. 将程序和统一品牌图标安装到临时 AppDir，桌面入口改为可迁移的可执行文件名与图标名。
3. 使用固定版本的 linuxdeploy 和 Qt 插件收集动态库、QML、翻译、图像和平台插件；手工收集的嵌套插件逐文件检查动态链接依赖。
4. 安装专用 AppRun，固定 Qt、QML、PipeWire 客户端模块的包内搜索目录，并保留调用者工作目录和音乐文件参数。
5. 校验运行文件清单、外部符号链接和字体文件；生成依赖许可证及源包版本记录。
6. 使用固定 AppImage runtime 生成 type-2 AppImage，执行重定位和自解包启动测试，成功后原子写入输出目录。

工具版本、下载地址和 SHA-256 记录于 `packaging/appimage/tools.lock.json`。下载经 HTTPS，缓存文件每次复核摘要；摘要错误在执行工具前终止。产物包含 `usr/share/liusheng/build-info.json`，记录应用版本、Qt 版本、架构及构建系统的 glibc 基线。

## 包内依赖与宿主边界

包内提供 Qt/QML、SVG、X11/Wayland 平台插件、PipeWire/SPA 客户端模块与 ALSA 配置。HarfBuzz、SM/ICE 等部分常被部署工具默认排除的客户端依赖也显式提供；额外的 Wayland shell 插件依赖纳入递归部署。

宿主提供 glibc、基本图形加载库、显卡驱动、系统字体、桌面合成器、桌面门户和音频服务。官方 AppImage 的 glibc 运行基线为 2.41；更早的系统需要额外的兼容构建。构建脚本从构建系统读取 glibc 版本并写入产物信息，避免在其他较新系统构建后继续标注旧基线。

AppDir 显式禁止开发机字体文件进入产物。运行时使用系统字体。PipeWire 服务、会话管理器和实际音频设备继续由宿主桌面管理。图标资源、音频引擎与曲库数据库结构沿用现有实现。

## 启动与配置

```sh
chmod +x liusheng-X.Y.Z-linux-x86_64.AppImage
./liusheng-X.Y.Z-linux-x86_64.AppImage
# FUSE 不可用时采用 runtime 的解包运行模式
./liusheng-X.Y.Z-linux-x86_64.AppImage --appimage-extract-and-run
# 手工解包，可用于检查或替换动态依赖
./liusheng-X.Y.Z-linux-x86_64.AppImage --appimage-extract
./squashfs-root/AppRun
```

AppImage 与系统安装版本共用应用 ID、MPRIS 名称、单实例规则和用户配置目录。切换到新版本前，通过 Ctrl+Q 完全退出旧实例。解包运行会有额外的磁盘开销，正常桌面可使用 FUSE 挂载运行。

AppRun 将 Qt 插件与 QML 搜索路径固定到包内，支持带空格的迁移路径。Wayland 会话默认选择 Wayland，并保留 X11 后备；调用者明确设置的 `QT_QPA_PLATFORM` 优先生效。`HOME`、XDG 用户数据位置和相对音乐文件路径保留调用者语义。ALSA 配置使用独立继承文件描述符处理包目录中的空格，避免 ALSA 将文件路径拆分成多个配置项；显式用户配置继续优先。

## CI 与发布门禁

`check.yml` 和 `release.yml` 在各自提交上调用相同的 `quality.yml`，完整执行 Linux 质量／界面／Wayland 回归和 macOS arm64 原生检查。两条入口同时共用 AppImage 工作流，其包含两个阶段：

- **构建与包内验证**：实际生成 AppImage，检查所有插件的动态链接依赖，执行带空格重定位、自解包、完整页面 / 功能，以及 Weston Wayland 100% / 125% / 150% 回归。
- **独立运行环境**：使用新的 Debian 13 容器，仅安装基本图形与系统库，在没有系统 Qt 的环境中重复运行包内容及 AppImage runtime。

`release.yml` 先通过完整共享质量门禁，再构建四种产物。DEB 与 RPM 分别进入新的 Debian/Fedora 容器，安装实际附件并检查安装后的版本、启动器、图标及 Qt 场景。macOS ZIP 解包迁移后检查架构、签名与初始 QML。发布 job 等待四种构建及 DEB/RPM 运行检查全部通过，仓库写权限限定在发布 job。发布附件检查拒绝缺失、重复、空文件、符号链接、混入旧版本及退出支持范围的产物。四个附件验证成功后生成并复核 SHA256SUMS，再创建 Release。

测试入口：

```sh
just package-contract-test
python3 scripts/check-appimage.py dist/liusheng-X.Y.Z-linux-x86_64.AppImage \
  --full-ui --wayland --output target/qa/appimage/validation.json
```

可移植测试覆盖发布目标、版本、校验值、工具缓存、构建失败、目标架构、AppDir 必需文件、路径逃逸、相对路径、Wayland 选择和调用者配置。原生 macOS 打包仍由 macOS CI 验证。

## 0.3.2 的历史本地验证口径

日志与产物集中于 `target/qa/release-targets/final/`。运行环境缺少挂载新 proc 文件系统的权限，因此最小宿主测试将 AppImage 在外部解包后放入独立 chroot，在其中检查每个 Qt/QML/音频插件并启动程序；chroot 的 `/usr` 包含独立下载的 Debian 基础图形 / 系统包，Qt 和 PipeWire 安装均为空。AppImage 自解包 runtime 在开发容器另行实际运行。CI 的独立容器覆盖完整 runtime 路径。

本轮还在普通构建环境中验证完整 Rust 测试、严格 Clippy、QML 控件、Wayland 界面行为和工作流语法。实机 GPU、桌面门户、物理音频设备与 FUSE 挂载体验属于桌面验收范围。

### 0.3.2 历史结果

本地完成 125 项 Rust 回归、60 项 Python 回归（含四目标发布和 AppImage 边界）、严格 Clippy、格式检查与三份工作流的 actionlint。Apple Silicon 核心库、示例和测试的目标编译检查通过。

已实际生成 62.67 MiB 的 x86_64 AppImage。在独立最小 chroot 中检查 128 个 ELF/插件并成功启动，系统 Qt 和 PipeWire 库均未安装。带空格路径的 AppRun 配置与 ALSA null PCM 打开测试已通过；独立 PipeWire null sink 下建立双声道链接，播放位置推进、暂停及退出验证通过。物理声卡仍需实机验收。

最终打包日志与测试结果以 `target/qa/release-targets/final/packaging-accepted.log`、`accepted-validation.json`、`clean-accepted.log`、`audio-accepted.log`、`python-accepted.log` 和 `rust-validation.log` 为准。已发布的 v0.3.1 保持原样，v0.3.2 的远程 CI/发布在推送后执行。

## 发布新版本

更新两个 crate 版本和 workspace 锁文件，提交并推送 main，等待该提交的完整质量检查与 AppImage 检查成功。为同一提交创建 `vX.Y.Z` 附注标签并推送。标签版本须与源码版本一致；Release 工作流会再次在标签对应提交执行共用质量门禁、实际打包与安装验证。

发布完成标准为四种安装包与 `SHA256SUMS` 齐全，Publish GitHub Release 成功。历史标签保持原提交。当前优化迭代的证据见 [OPTIMIZATION_ITERATION.md](OPTIMIZATION_ITERATION.md)。

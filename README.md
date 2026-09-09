# Liusheng 留声

Linux 与 macOS 本地音乐播放器。Linux 版提供 PipeWire 共享输出和 ALSA 独占输出，macOS 版使用 CoreAudio 共享输出。

技术结构：`liusheng-core` 是无 UI 依赖的 Rust 核心（解码、播放引擎、曲库），前端用 Qt Quick/QML，经 cxx-qt 桥接。设计决策全文见 [DECISIONS.md](DECISIONS.md)。

![留声专辑墙](docs/images/liusheng-albums.png)

## 界面

Quiet Library 界面采用暖白 / 炭黑配色、统一线性图标与精简的曲库侧栏。队列与输出设备分别收进侧抽屉和弹层；正在播放页与曲库共用播放栏。设置支持跟随系统、浅色、深色和减少动画。

歌曲支持双击 / Enter 播放、更多菜单及键盘操作；专辑与艺术家均支持局部搜索。窄窗口自动收起侧栏文字，歌曲表格合并次级信息。

设计规范与操作说明见 [GUI_DESIGN.md](docs/GUI_DESIGN.md)。文档截图来自隔离的虚构测试曲库。

## 视觉与 Wayland 打磨

新版提供紧凑 / 舒适封面布局，统一导航与设置对齐，提升文字可读性，并修正实际网格列数、图标拉伸和宽屏歌词栏尺寸问题。应用内菜单使用窗口内弹层；Wayland 托盘右键操作在主窗口中显示兼容菜单。已有关闭到托盘偏好继续生效，新建 Wayland 配置默认关闭即退出。

Groove 图标族含 34 个新绘制字形、应用 SVG、多尺寸 PNG、单色变体和 macOS ICNS。安装与卸载脚本已覆盖各尺寸桌面图标。

验证入口为 `just ui-controls-test`、`just ui-test`、`just wayland-test`、`just icons-check`。Wayland 测试使用独立 Weston 合成器和 100% / 125% / 150% 缩放；详见 [UI_POLISH_WAYLAND.md](docs/UI_POLISH_WAYLAND.md)。

## 聆听页与系统媒体控制

“正在播放”提供封面与歌词、聚焦歌词、纯封面三种布局。封面配色经过明暗与可读性约束；同时间戳的原文和附文一起显示，手动阅读后可回到当前句。展开封面使用窗口内连续转场，背景在暂停、隐藏及减少动画时停止。

macOS 通过公开 MediaPlayer 接口发布歌曲、封面、进度与播放状态，并接收系统播放、暂停、切歌和定位命令。Linux 继续使用 MPRIS。原生系统控制回归在 Apple Silicon CI 中执行；控制中心与耳机按键的最终体验需要 macOS 桌面验收。

设计、数据协议和验证范围见 [LISTENING_AND_MACOS_MEDIA.md](docs/LISTENING_AND_MACOS_MEDIA.md)。Apple Silicon 可运行 `just macos-media-test`；Linux 继续运行 `just ui-test` 与 `just wayland-test`。

## 构建

```sh
just build     # 构建
just test      # 测试
just run       # 启动桌面应用
just install   # 构建 release 并安装到 ~/.local
just uninstall # 卸载程序，保留曲库数据
just alsa-probe # 用静音验证 AKG N9 的原生格式与重采样
just output-smoke # 验证共享、独占、共享输出切换
just volume-probe # 验证 AKG N9 的硬件音量与静音开关
just package-deb # 生成 amd64 DEB
just package-rpm # 生成 x86_64 RPM
just package-appimage # 生成 Linux x86_64 AppImage（Python 3.11+，官方构建环境 Debian 13）
just package-macos # 生成 Apple Silicon arm64 应用包
```

个人安装在普通用户终端执行 `just install`，默认安装到该用户的 `~/.local`。安装完成后再启动：

```sh
just install && ~/.local/bin/liusheng
```

安装、卸载与 Shell 打包 recipe 显式调用 Bash；AppImage recipe 显式调用 Python，支持复制或编辑后脚本执行位丢失的工作区。直接调用安装器的等价命令是 `bash ./scripts/install.sh`；脚本在 Git 中保留 `100755` 执行权限。

托盘直接加载内嵌品牌图标；启动器使用带内容校验值的绝对 SVG 路径。安装器会打印图标路径和程序 SHA-256。出现旧图标时，在受影响的桌面运行 `just icons-diagnose`，核对运行进程、安装文件和重复的启动入口。

安装脚本支持 `PREFIX` 和 `DESTDIR`。打包测试示例：

```sh
PREFIX=/usr DESTDIR=/tmp/liusheng-package just install
```

Linux 首次启动优先使用现有 `/data/Music`，否则使用 `~/Music`。可在设置中添加多个音乐目录和排除目录。Fedora 构建依赖：

```sh
sudo dnf install qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtsvg qt6-qtwayland clang pipewire-devel alsa-lib-devel just flac
```

macOS 版从 `~/Music` 扫描音乐。安装 Qt 后可直接构建应用包：

```sh
brew install qt
just package-macos
```

## 开发工具与跨平台验证

`dev` 示例在 Linux 使用 PipeWire，在 macOS 使用 CoreAudio；扫描、搜索和 WAV 解码在两个平台上共用实现。ALSA 独占与硬件音量探测限定为 Linux。`just dev-cli-test` 构建并执行硬件无关的命令回归，覆盖 16/24 位解码、目录扫描及搜索。

普通 CI 覆盖 Linux、macOS arm64 与 AppImage 构建 / 独立运行环境。Linux 检查 aarch64-apple-darwin 目标的核心库、示例与测试编译；原生 macOS job 执行完整 workspace 测试、开发命令检查及 Qt 初始场景加载。现行发布策略见 [RELEASE_TARGETS.md](docs/RELEASE_TARGETS.md)，历史修复背景见 [MACOS_CI_FIX.md](docs/MACOS_CI_FIX.md)。

文件监听回归通过 `just watcher-test` 重复执行：防抖合并使用可控时间验证，原生文件系统测试验证曲库最终状态。文件与目录通知、分批送达、重扫标记和路径容量均有覆盖；详见 [WATCHER_CONTRACT_FIX.md](docs/WATCHER_CONTRACT_FIX.md)。每轮均须通过，首次失败立即退出。

## 检查更新

`设置 → 关于与更新` 提供手动检查和“启动时自动检查更新”开关。启动首帧与缓存曲库就绪后延迟 5 秒检查 GitHub 最新正式 Release；发现较新版本时显示轻提示，支持稍后提醒、跳过此版本和恢复提醒。

更新详情包含版本、日期、说明和适用平台的安装包；下载在用户点击后交给浏览器，安装沿用原方式。网络失败保持安静，检查结果可随时在设置中查看。后台检查仅发送正常 GitHub 元数据请求，曲库和播放记录保留在本机。

长时间保持应用开启时可手动检查新版本。`just updates-test` 运行隔离网络回归，`--no-update-check` 为本次运行关闭更新联网。设计、隐私和发布契约见 [UPDATES.md](docs/UPDATES.md)。

## 发布

正式产物为 **DEB、RPM、AppImage、macOS arm64 ZIP**，加一份 `SHA256SUMS`。Linux 三种产物均为 x86_64；Apple Silicon 使用 arm64。历史版本的附件保持原样。Arch 原生包和 Intel Mac 已退出自动构建与发布。

```sh
bash scripts/package.sh deb
bash scripts/package.sh rpm
python3 scripts/package-appimage.py
bash scripts/package-macos.sh
```

DEB 使用 Debian 13，RPM 使用 Fedora 44 构建。AppImage 使用 Debian 13 构建，面向 **glibc 2.41+** 的 Linux x86_64 桌面，内置 Qt/QML、SVG、X11/Wayland 平台插件和音频客户端模块；使用系统字体、显卡驱动与音频服务。macOS arm64 ZIP 采用临时签名，首次运行可通过 Finder 右键“打开”。

```sh
chmod +x liusheng-0.3.2-linux-x86_64.AppImage
./liusheng-0.3.2-linux-x86_64.AppImage
# FUSE 不可用时采用解包运行：
./liusheng-0.3.2-linux-x86_64.AppImage --appimage-extract-and-run
```

AppImage 与已安装的留声共用应用 ID、单实例规则和用户配置；切换版本前先通过 Ctrl+Q 退出旧实例。程序在当前目录之外运行时，也会保留传入的相对音乐文件路径。

普通 CI 与标签发布共用 `.github/workflows/package-appimage.yml`。发布前核对四种附件的数量、版本及 SHA-256，全部通过后创建 GitHub Release。`just package-contract-test` 验证发布目标、版本、工具校验值、启动环境和路径处理。构建工具及 AppImage runtime 固定版本与 SHA-256，记录在 `packaging/appimage/tools.lock.json`。

推送通过 CI 的提交后，为该提交创建 `vX.Y.Z` 标签；标签版本须与两个 crate 和 workspace 锁文件一致。当前准备版本为 `0.3.2`。详细构建、验证边界与发布步骤见 [RELEASE_TARGETS.md](docs/RELEASE_TARGETS.md)。

## 0.3.0 功能与性能升级

启动先展示 SQLite 缓存曲库，目录校验、搜索、封面和硬件音量通过独立工作线程处理。首次导入分批显示结果；日常文件变化按路径更新，离线目录及扫描失败范围保留缓存。

支持持久化歌单、M3U8 导入导出、队列拖动与按钮排序、随机播放、单曲/列表循环。退出保存队列、位置、窗口与浏览页面；下次启动恢复为暂停状态。设置支持多目录、排除目录、ALSA 设备选择、硬件混音器与关闭行为。文件管理器、命令行、拖入文件和 MPRIS OpenUri 共用本地文件入口；第二次启动将请求发送给已有实例。

封面采用按需分档缩略图并缓存主题色；歌词支持每曲偏移和 LRC 文件变化刷新。信号链面板显示已配置的输出后端信息，并区分应用提交格式与系统最终设备协商。

```sh
just ui-test               # 隔离环境中的页面与功能回归（Linux，需 python3/dbus-run-session/gdbus）
just startup-bench         # release 合成缓存曲库启动基准
just audio-contract-test  # Linux 上的音频接口与缓冲回归
```

`LIUSHENG_PROFILE=1 ./target/release/liusheng` 输出启动与扫描阶段标记。详细架构、数据迁移、测试范围及已知边界见 [docs/UPGRADE_0_3.md](docs/UPGRADE_0_3.md)。

## 既有能力

第一阶段（MVP）功能已贯通。Rust 核心具备解码、播放、曲库增量扫描、目录变更监听与拼音搜索。Qt/QML 桌面应用支持专辑、艺术家、全部歌曲和播放队列浏览，支持搜索、播放控制、歌词、封面和系统托盘。Linux 版提供 PipeWire 共享输出、MPRIS、ALSA 独占输出和 AKG N9 硬件音量控制。macOS 版提供 CoreAudio 共享输出，界面会隐藏 Linux 专属控件。

独占模式按 ALSA 实际协商结果输出整数 PCM；设备支持 44.1 kHz 时保留原生采样率，N9 等设备需要适配时使用 Rubato 将 44.1 kHz 连续重采样到 96 kHz、24 位。专辑封面优先读取音频内嵌图片，回退到同目录的 `cover`、`folder`、`front` 图片。当前源码未授予开源许可证，权利保留。

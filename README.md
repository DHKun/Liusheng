# Liusheng 留声

Linux 与 macOS 本地音乐播放器。Linux 版提供 PipeWire 共享输出和 ALSA 独占输出，macOS 版使用 CoreAudio 共享输出。

技术结构：`liusheng-core` 是无 UI 依赖的 Rust 核心（解码、播放引擎、曲库），前端用 Qt Quick/QML，经 cxx-qt 桥接。设计决策全文见 [DECISIONS.md](DECISIONS.md)。

![留声专辑墙](docs/images/liusheng-albums.png)

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
just package-arch # 生成 x86_64 Arch Linux 包
just package-macos # 生成当前 Mac 架构的应用包
```

安装脚本支持 `PREFIX` 和 `DESTDIR`。打包测试示例：

```sh
PREFIX=/usr DESTDIR=/tmp/liusheng-package just install
```

Linux 首次启动优先使用现有 `/data/Music`，否则使用 `~/Music`。可在设置中添加多个音乐目录和排除目录。Fedora 构建依赖：

```sh
sudo dnf install qt6-qtbase-devel qt6-qtdeclarative-devel clang pipewire-devel alsa-lib-devel just flac
```

macOS 版从 `~/Music` 扫描音乐。安装 Qt 后可直接构建应用包：

```sh
brew install qt
just package-macos
```

## 发布

`scripts/package.sh` 将安装文件放入系统标准路径，并把安装包写入 `dist/`：

```sh
./scripts/package.sh deb
./scripts/package.sh rpm
./scripts/package.sh arch
./scripts/package-macos.sh
```

DEB 以 Debian 13 为运行基线，RPM 以 Fedora 44 为运行基线。Arch 包需要在 Arch Linux 普通用户环境中运行 `makepkg`。macOS ZIP 包经过临时签名，首次运行时需在 Finder 中右键选择“打开”。

推送 `vX.Y.Z` 标签后，GitHub Actions 会生成 DEB、RPM、Arch x86_64、macOS arm64 和 macOS x86_64 产物，并发布 SHA-256 校验文件。标签版本必须与 `crates/liusheng/Cargo.toml` 一致。

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

# Liusheng 留声

Linux 本地音乐播放器。为无损 FLAC 收藏服务，声音路径不做任何加工，界面自定义设计并以动画质量为目标。

技术结构：`liusheng-core` 是无 UI 依赖的 Rust 核心（解码、播放引擎、曲库），前端用 Qt Quick/QML，经 cxx-qt 桥接。设计决策全文见 [DECISIONS.md](DECISIONS.md)。

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
```

安装脚本支持 `PREFIX` 和 `DESTDIR`。打包测试示例：

```sh
PREFIX=/usr DESTDIR=/tmp/liusheng-package just install
```

前端部分依赖 Qt6 开发包，Fedora 上安装：

```sh
sudo dnf install qt6-qtbase-devel qt6-qtdeclarative-devel clang pipewire-devel alsa-lib-devel just flac
```

## 发布

`scripts/package.sh` 将安装文件放入系统标准路径，并把安装包写入 `dist/`：

```sh
./scripts/package.sh deb
./scripts/package.sh rpm
```

DEB 以 Ubuntu 24.04 为运行基线，RPM 以 Fedora 44 为运行基线。推送 `v0.1.0` 形式的标签后，GitHub Actions 会在对应系统中构建两个安装包，生成 SHA-256 校验文件并创建 GitHub Release。标签版本必须与 `crates/liusheng/Cargo.toml` 一致。

## 状态

第一阶段（MVP）功能已贯通。Rust 核心具备解码、播放、PipeWire 输出、曲库增量扫描、目录变更实时监听与拼音搜索；Qt/QML 桌面应用支持专辑浏览、曲目点播、暂停、继续、切歌、播放进度定位、MPRIS 系统媒体控制和系统托盘驻留。`just install` 可将 release 版本、desktop 启动项和应用图标安装到 `~/.local`。第二阶段已加入 ALSA 独占输出 adapter，支持 AKG N9 的双声道 48/96 kHz、16/24 位原始格式，并可在界面中切换 PipeWire 共享输出与 ALSA 独占输出、恢复当前播放状态。独占模式使用 Rubato 高质量 sinc 将 44.1 kHz 连续重采样到 96 kHz / 24 位，48/96 kHz 内容保持整数路径逐比特直通。底部播放条可控制 AKG N9 的硬件音量与静音开关，PCM 样本保持原值。播放器支持同名 LRC 和音频内嵌歌词，沉浸播放页会随播放位置滚动同步歌词。专辑封面优先读取音频内嵌图片，回退同目录 `cover`、`folder`、`front` 图片，并通过版本化缓存同步显示在专辑墙、播放条、沉浸页和系统媒体控件。当前源码未授予开源许可证，权利保留。

# Liusheng 留声

Linux 本地音乐播放器。为无损 FLAC 收藏服务，声音路径不做任何加工，界面自定义设计并以动画质量为目标。

技术结构：`liusheng-core` 是无 UI 依赖的 Rust 核心（解码、播放引擎、曲库），前端用 Qt Quick/QML，经 cxx-qt 桥接。设计决策全文见 [DECISIONS.md](DECISIONS.md)。

## 构建

```sh
just build   # 构建
just test    # 测试
just run     # 启动桌面应用
```

前端部分依赖 Qt6 开发包，Fedora 上安装：

```sh
sudo dnf install qt6-qtbase-devel qt6-qtdeclarative-devel clang pipewire-devel alsa-lib-devel just flac
```

## 状态

第一阶段（MVP）开发中。Rust 核心已具备解码、播放、PipeWire 输出、曲库扫描与拼音搜索，Qt/QML 桌面骨架已可启动。MPRIS、托盘和安装链路继续开发。许可证在开源发布前确定。

# Liusheng 设计决策记录

日期：2026-07-26。2026-08-13 增加 macOS 构建范围。本文记录项目核心决策，每条含结论与理由。变更决策时在此文件更新并注明日期。

## 项目定位

Liusheng 的完整功能以 Linux 为准，设计环境是 Fedora 44、KDE、Wayland 和 PipeWire 1.6.8。macOS 版提供 CoreAudio 共享输出和可下载应用包。ALSA 独占输出、硬件音量和 MPRIS 保留为 Linux 功能。代码托管到 GitHub（用户名 DHKun）。

应用 ID：`io.github.dhkun.Liusheng`。desktop 文件、MPRIS、托盘统一用它。

## 音乐来源

只支持本地目录。Linux 曲库根目录为 `/data/Music`，macOS 曲库根目录为 `~/Music`。不做网络协议、流媒体和设备同步。网络存储由文件系统挂载。

## 声音路径

声音哲学：文件里是什么就送出什么，不做任何加工。因此不做 EQ、不做 ReplayGain、不做软件音量。

硬件事实约束：当前输出设备 AKG N9 Hybrid 只接受 48kHz 和 96kHz，不接受 44.1kHz。CD 抓轨的 44.1kHz 内容必须重采样后才能输出，48kHz 和 96kHz 内容可以逐比特直通。

### 输出模式

双模式，界面一键切换：

1. 共享模式：作为 PipeWire 原生客户端输出，与其他应用共存，日常使用。
2. 独占模式：绕过 PipeWire，直接以 ALSA hw 设备独占输出。48k、96k 内容逐比特直达硬件，44.1k 内容由应用内重采样器处理。播放期间其他应用无法使用该设备，需处理 WirePlumber 的设备释放协调。

实现顺序：先共享模式，独占模式放第二阶段。

### 独占模式音量

硬件音量为主：通过 ALSA mixer 控制 USB 设备自身的音量，数字流保持 bit-perfect。设备不暴露硬件音量控制时，音量条锁定 100% 并置灰，音量由耳机按键调节。不做软件音量。

### 重采样

用 rubato 的高质量 sinc 档，纯 Rust。若试听不满意，换 libsoxr 绑定。44.1k 内容重采样到 96k 输出。

### 格式支持

解码层用 Symphonia。FLAC 是一等公民，所有优化围绕它。MP3、AAC、OGG、ALAC、WAV 由 Symphonia 顺带支持。不支持 APE 和 DSD。

### 播放正确性

gapless 是硬性要求，做进引擎底层：预解码下一曲，样本级拼接。

## 技术栈

核心是无 UI 依赖的 Rust crate `liusheng-core`，包含播放引擎、解码、曲库、数据库。前端用 Qt Quick/QML，通过 cxx-qt 桥接。

选 QML 的理由：GPU 场景图和声明式动画适合本项目对动画的要求，KDE 上 Qt 库现成、外观协调，中文输入法支持可靠。Slint 和 Iced 的输入法与视觉效果生态不满足要求，Tauri 的 WebKitGTK 在 NVIDIA 加 Wayland 环境下有掉帧和闪烁问题。

## 界面

自定义设计语言，质感对标 Spotify、Apple Music、Cider。封面驱动：正在播放的封面是视觉中心，从封面取色渲染模糊渐变背景，界面色调随曲目切换。跟随系统深浅色模式。

结构为经典三区：左侧栏放资料库入口和歌单，主内容区展示列表与详情，底部常驻播放条，点击播放条展开全屏沉浸播放页。

页面共七个，不再增加：专辑墙（主视图）、专辑详情、艺术家、全部歌曲、歌单、播放队列、沉浸播放页。不做推荐、最近播放等流媒体式功能。

## 曲库

数据库用 SQLite。启动时增量扫描，运行中用 inotify 监听 `/data/Music`，文件变动即时入库。

搜索支持拼音：入库时把标题、艺术家预转全拼和首字母索引，输入 "ljj" 或 "linjunjie" 都能命中 "林俊杰"。

标签只读不写，不做标签编辑器，修标签用 MusicBrainz Picard 等外部工具。

整轨 FLAC 加 cue 的分轨支持列入第二阶段。收藏拷入 `/data/Music` 后检查实际情况，若收藏中没有 cue 文件则取消此项。

歌单存 SQLite，支持 m3u8 导入导出。

## 歌词

读内嵌标签（LYRICS、UNSYNCEDLYRICS）和同目录 `.lrc` 文件。沉浸播放页做平滑滚动歌词：当前行高亮，其余行虚化，切行带弹簧动画。

联网抓词做成设置里的可选功能，默认关闭，实现顺序排在本地歌词之后。歌词批量配齐建议用 LDDC 等外部工具。

## 桌面集成

MPRIS 必做，KDE 媒体弹窗、锁屏控制、媒体键、蓝牙耳机按键全部经由它。不单独做切歌通知。

关闭窗口时缩进系统托盘继续播放（KDE StatusNotifier），托盘菜单提供暂停、切歌和退出。设置里留 "关闭时退出" 开关。

## 构建与交付

自用阶段：`cargo build --release` 加安装脚本（`just install`），装二进制、desktop 文件、图标到 `~/.local`。

GitHub Release 提供 Debian 13 DEB、Fedora 44 RPM、Arch Linux x86_64 包，以及 macOS arm64 和 x86_64 应用包。macOS 包使用临时签名，取得 Apple Developer 证书后再加入公证。COPR 和 Flatpak 继续作为 Linux 发布选项。

## 路线图

1. MVP：解码、共享模式输出、gapless、曲库扫描与拼音搜索、三区界面骨架、MPRIS、托盘。
2. HiFi 完全体：ALSA 独占输出、硬件音量、采样率自动切换、cue 分轨、本地歌词滚动。
3. 打磨：氛围动效精修、联网抓词、开源准备（README、COPR）。


## 2026-09-07：0.3.0 功能与性能升级

- 曲库展示采用缓存优先；扫描与监听属于后台 LibraryService。SQLite 文件保持既有路径，schema v2 增加文件长度与索引，并复用已持久化的拼音搜索文本。目录离线、扫描失败和取消时保留未确认删除的记录。
- 封面由独立有界任务队列按页面需求生成 256/768 像素档位，版本索引持久化，过期文件集中清理。主题色随封面版本缓存。
- 播放引擎、库服务、搜索、封面、平台音量与 MPRIS 使用独立职责。Qt 模型在 GUI 线程提交行变化；音频回调使用预分配 SPSC PCM 队列。读盘和预加载保留在工作线程。
- ALSA 设备选择可配置，N9 作为默认预设。其他设备按原生 PCM 能力协商，44.1 kHz 原生支持通过探测及最终配置校验；需要适配时维持既有高质量 44.1→96 kHz 路径。
- 歌单独立保存文件路径与排序，允许同一曲目重复；会话默认恢复为暂停。随机与循环模式属于播放引擎并通过 MPRIS 同步。
- 本地文件入口包含命令行、拖放、文件选择器、MPRIS 和 macOS 文件打开事件。用户级单实例锁与本地 IPC 避免重复实例争抢输出和曲库。
- 质量门禁包含核心测试、Qt 模型测试、隔离 HOME 的离屏功能测试和原生 macOS 构建任务。容器基准单独标记离屏/缓存条件；硬件与桌面实测独立验收。


## 2026-09-07 · Quiet Library GUI 重构

界面采用四项曲库导航、共用播放栏、队列侧抽屉和输出弹层。所有页面通过 Theme 单例使用统一色彩与动效令牌；默认跟随系统，用户可以显式选择浅色或深色。Qt Quick Controls Basic 提供按钮、输入和菜单语义，局部自定义外观。旧的多色装饰组件由 Quiet 控件和共用曲目表格替代。

保持 AppController、音频引擎和数据模型的既有职责。新增 appearance / reduced_motion 设置字段使用默认值兼容旧配置。窗口、列表、对话框和弹层复用已存在的播放与持久化接口。验证增加鼠标/键盘控件测试、SVG 加载错误检查以及正常/紧凑窗口的浅深色截图。完整设计见 docs/GUI_DESIGN.md。

## 2026-09-07 · 视觉细节与 Wayland

所有应用内菜单明确使用 Popup.Item，并将触发坐标映射到当前窗口 Overlay 后做边界约束。Wayland 模式保留托盘图标，原生 Platform.Menu 工厂保持未实例化，右键通过主窗口显示兼容操作；其他平台保留原托盘菜单。已有配置不改变，新建 Wayland 配置默认关闭即退出。文件/目录对话框明确关联 parentWindow。

Groove 图标源与生成器归项目维护，SVG 使用 qrc 资源路径，IconImage 着色适配隔离在 Icon.qml。封面占位、提取与缓存代码保持本轮开始时的内容。新增 compact_grid 偏好默认紧凑；控件、页面与原生 Wayland 三档缩放加入回归。详细验收记录在 docs/UI_POLISH_WAYLAND.md。


## 2026-09-08 · 四目标发布与 AppImage

正式发布收敛到 Linux x86_64 的 DEB、RPM、AppImage，以及 macOS arm64 ZIP。移除自动 Arch 打包任务与 Intel macOS job/交叉检查，保留既有版本附件和历史修复记录。旧 Arch 手工脚本保留为历史工具。

AppImage 使用 Debian 13、Qt 6.8+ 构建，运行基线为 glibc 2.41+。锁定 linuxdeploy、Qt 插件、appimagetool 和 type-2 runtime 的版本与 SHA-256。明确收集 Qt/QML、Wayland 和音频客户端模块，保留宿主 GPU、字体、PipeWire 服务及用户配置。普通 CI 与发布复用同一 AppImage 打包和运行检查工作流；四种附件全部通过校验后进入发布步骤。实现与验证范围见 docs/RELEASE_TARGETS.md。


## 2026-09-09 · 聆听语义主题与 macOS MediaPlayer

当前播放内容使用独立的语义色角色和可读性约束，曲库保持原有稳定主题。歌词展示单元与原始时间轴分离，同时间戳文本组合但保留原始数据；虚拟列表承担布局，后台线程完成读取与序列化。封面转场和菜单都保留在当前 Qt Quick 窗口内。

macOS 使用公开 MediaPlayer 框架完成 Now Playing 元数据和远程命令注册。平台适配器只把命令投递给现有控制器，音频引擎继续拥有播放状态。恢复的暂停会话不主动声明系统媒体播放归属，实际播放后发布，停止/退出时清理。原生适配器在 macOS arm64 CI 中独立编译和测试。

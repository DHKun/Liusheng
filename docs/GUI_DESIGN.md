# 留声 GUI · Quiet Library

本文件记录首版 Quiet Library 重构。后续视觉令牌、菜单策略、网格和图标规则以 [UI_POLISH_WAYLAND.md](UI_POLISH_WAYLAND.md) 为准。

## 设计方向

音乐收藏优先，界面保持安静。默认跟随系统外观，浅色采用暖白底与炭灰文字，深色采用近黑底与暖灰文字。单一棕色强调色表达选择和键盘焦点；专辑封面承担主要色彩与视觉内容。

参考的成熟交互模式：Apple Music 的曲库侧栏与专辑组织，Spotify 的曲库内搜索、筛选和更多操作菜单。留声继续聚焦本地音乐，保留自己的名称、图标和音频输出行为。

参考资料：
- Apple Music：<https://support.apple.com/guide/music/customize-the-music-window-mus0cec331d6/mac>
- Spotify Your Library：<https://support.spotify.com/us/article/your-library/>
- Qt Quick Controls 自定义：<https://doc.qt.io/qt-6/qtquickcontrols-customize.html>

## 信息架构

| 区域 | 入口与行为 |
| --- | --- |
| 常驻导航 | 专辑、艺术家、歌曲、歌单四项；设置位于侧栏底部 |
| 专辑/艺术家 | 封面网格、局部搜索、详情页。屏幕内封面按需请求 |
| 歌曲 | 标准歌曲表格，排序/格式筛选收进筛选弹层 |
| 队列 | 播放器右侧入口展开侧抽屉；支持播放、移除、拖动排序与上移/下移菜单 |
| 输出 | 播放器输出图标展开弹层，包含共享/独占、硬件音量和音频详情 |
| 正在播放 | 左封面右歌词；窄窗口切换封面/歌词。共用常驻播放器 |
| 设置 | 曲库、播放、外观与行为三个分组；保存与取消语义保持清晰 |

低频操作放入更多菜单；错误、空曲库和空搜索结果保留清晰可见的解释与操作入口。删除歌单、清空队列使用确认对话框。

## 视觉与组件规范

颜色、字体、动效时长集中在 `qml/Theme.qml`。正文、辅助文字和强调色在浅色/深色下有对比度回归测试。字体采用应用/系统字体；小圆角用于交互控件，封面保留方形边界。

页面标题通常为 26px，歌曲标题为 13px，辅助信息为 11–12px。默认正文行高 56px，队列行高 64px。常用按钮高度 38px，紧凑按钮高度 32px。键盘焦点使用明确的强调色描边。

默认窗口 1280×800，最小窗口 820×560；宽度小于 1000px 时侧栏收为图标导航。专辑列数随可用宽度变化，歌曲在较窄布局中把艺术家放到第二行。

图标使用本项目绘制的 SVG 路径与统一线宽。运行依赖 Qt SVG 图像插件：Fedora `qt6-qtsvg`、Debian `qt6-svg-plugins`、Arch `qt6-svg`，已写入对应打包依赖。macOS 使用完整 Qt 安装与 `macdeployqt`。

## 工程结构

`Main.qml` 负责窗口、导航、快捷键和弹层装配。页面与控件拆分如下：

| 文件 | 职责 |
| --- | --- |
| `Theme.qml`、`Icon.qml` | 视觉令牌、统一图标 |
| `Quiet*.qml` | 基于 Qt Quick Controls Basic 的按钮、输入、菜单、滑杆、选择框、对话框 |
| `LibraryPage.qml`、`CollectionView.qml` | 曲库页面编排与虚拟化封面网格 |
| `DetailPage.qml`、`TrackTable.qml` | 共用详情页和歌曲表格 |
| `PlayerBar.qml`、`QueuePanel.qml`、`OutputPopover.qml` | 常驻播放控制、队列和音频输出 |
| `ImmersivePlayer.qml`、`SettingsDialog.qml`、`PlaylistsPage.qml` | 正在播放、设置与歌单 |
| `ValidationHarness.qml` | 显式测试模式启用的页面/功能验收流程 |

旧的专辑卡片、唱片装饰、导航按钮、硬件音量行和重复歌曲行组件由上述组件替代。保留原 `AppController`、`QAbstractListModel` 和音频引擎协议。过滤后的激活始终使用模型的 `sourceIndex`。

新增 `AppSettings.appearance` 和 `AppSettings.reduced_motion`，通过 serde 默认值兼容已有设置文件。主题选择为 `system`、`light`、`dark`；改动仅涉及设置字段，曲库数据库沿用现有结构。

封面、歌词、歌曲表格和队列的实际数据继续来自现有服务。艺术家列表、歌曲列表、详情、歌单、设置、队列和正在播放页面按首次使用加载。封面异步解码，歌词区域保持列表虚拟化。界面采用短暂过渡，减少动画设置可关闭这些过渡。

## 操作约定

- 单击歌曲选择；双击或 Enter 播放；悬停播放按钮与更多菜单提供直接入口。
- 更多菜单通过鼠标右键、更多按钮或 Shift+F10 / Menu 键打开；队列也有上下移动命令。
- Ctrl+O 打开文件，Ctrl+F 搜索，Ctrl+J 队列，Ctrl+L 正在播放，Ctrl+, 设置，Alt+Left 返回曲库详情的上一级。
- Space 在浏览场景控制播放，输入框与具有自身激活语义的控件保留键盘行为。
- 托盘、文件拖入、文件管理器打开、MPRIS、会话恢复继续使用原有入口。

## 验证与截图

```sh
just ui-test
just ui-controls-test
just audio-contract-test
just startup-bench
just ui-preview
```

`ui-controls-test` 需要 Qt Quick Test 模块与 SVG 插件，实际发送鼠标/键盘事件，检查按钮、输入框、选择框、滑杆、对话框、歌曲源索引与主题对比度。图标加载失败、QML 绑定循环、类型错误和未定义引用会使回归检查失败。

`ui-test` 使用隔离 HOME、XDG 目录、临时曲库与 D-Bus 会话，覆盖浅色/深色、正常/紧凑窗口、专辑、艺术家、歌曲、队列、歌单、设置、正在播放、搜索空结果与输出弹层。原有播放恢复、队列、歌单导入导出和 MPRIS 功能回归继续执行。

`ui-preview` 额外需要 Pillow，在临时目录生成原创几何封面和虚构曲库，再截取实际程序窗口。预览图片仅展示测试数据。源码中的 `docs/images/liusheng-albums.png` 等图片由这条路径生成。

本次验证日志位于 `target/qa/gui-validation/`；设计预览位于 `target/qa/gui-preview/`；窗口边界和空状态截图位于 `target/qa/gui-refactor/`。启动前后对比采用同一容器、release、离屏软件渲染、预热缓存和合成离线曲库；它衡量应用内界面阶段，真实桌面冷启动、Wayland、屏幕阅读器与原生 macOS 仍需实机验收。

## 本次验证结果

Rust workspace + CoreAudio 接口检查共 95 项测试通过；9 项 Qt Quick 控件测试通过（测试框架另计初始化/清理两项）。严格 Clippy、Rust 格式检查、release 构建及 release 的界面/功能回归通过。额外执行了 150% 缩放的同一套界面/功能回归。

同条件七次采样中位数，应用内首帧 / 可交互标记：

| 合成曲库 | 重构前首帧 | 重构后首帧 | 重构前可交互 | 重构后可交互 |
| --- | ---: | ---: | ---: | ---: |
| 1,000 首 | 24.90 ms | 14.31 ms | 67.92 ms | 29.31 ms |
| 10,000 首 | 23.88 ms | 14.77 ms | 95.97 ms | 68.82 ms |

测量环境：Debian 13 容器，Qt 6.8.2，软件离屏渲染，预热缓存。应用内标记从 Qt 桥接计时器开始，进程创建和桌面合成器时间另计。原始样本保存在 target/qa/gui-validation/startup-before.json 和 startup-after.json。此表用于比较本次代码改动，真实 Fedora 桌面冷启动应单独测量。

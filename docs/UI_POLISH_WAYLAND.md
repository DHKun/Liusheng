# GUI 细节打磨、Wayland 与 Groove 图标

日期：2026-09-07。基于工作区现有的 Quiet Library GUI 继续修改，曲库和音频引擎保持原样。

## 实际修正

| 区域 | 改动与回归点 |
| --- | --- |
| 导航 | 新建 NavigationItem，资料库入口与设置共用图标列、文字起点、选中态和键盘焦点。正常侧栏 200px，紧凑侧栏 72px |
| 字体与留白 | 统一 Theme 的 4/8/12/16/24/32/40 间距、26/18/14/13/12/11 字号、90/140/200ms 动效，主次文字对比度有自动检查 |
| 专辑与艺术家 | 默认紧凑网格，设置提供舒适模式；修正列宽计算导致实际少排一列的问题。卡片标题与艺术家各自保留固定文字区；长文本省略后可悬停查看 |
| 歌曲与队列 | 使用更易读的主次文字；时间图标改为时钟；修正宽布局单元格将图标拉伸的问题。队列拖动有独立 24px 手柄；右键、Shift+F10、移动与取消焦点都有回归 |
| 正在播放 | 修正宽屏布局把歌词栏压为零宽的缺陷，封面与歌词均有明确宽度约束。当前歌词使用固定字号和字重，通过颜色表达状态；窄屏保留封面/歌词切换 |
| 菜单与对话框 | 统一 Item 弹层、边界钳制、轻量九宫格阴影、Esc 与外部点击语义。嵌套选择框优先关闭子弹层；设置取消保留原配置；修正命名/确认对话框头尾高度挤占正文的问题，输入框保留完整高度 |
| 输出面板 | 长设备名称、错误详情与信号链内容在有限高度内滚动，主窗口保持可操作 |
| 滚动提示 | 内容溢出时显示细滚动条，单页可完整展示时隐藏滑块 |

占位封面组件 `CoverArt.qml`、封面提取 `artwork.rs`、封面服务 `artwork_service.rs` 与播放引擎 `engine.rs` 已与本轮开始时备份逐字节比较，保持一致。

## Wayland 策略

### 应用内弹层

QuietMenu、QuietPopover、QuietDialog、队列抽屉、选择框与提示均显式使用 `Popup.Item`。菜单统一通过 `anchor.mapToItem(Overlay.overlay, ...)` 将触发点映射到窗口内坐标，再按窗口边界约束位置。

选择框的弹层保留与控件关联的定位关系，通过 `Popup.Item` 在同一窗口场景中绘制；界面内菜单和普通弹层使用 Overlay 作为定位父级。文件与目录对话框使用 Qt Quick Dialogs 并显式指定 `parentWindow`。代码兼容 Qt 6.8，未使用 Qt Quick Dialogs 6.10 才引入的 popupType 属性。

参考：[Qt Popup 类型与位置](https://doc.qt.io/qt-6/qml-qtquick-controls-popup.html)、[Qt Quick Dialogs parentWindow](https://doc.qt.io/qt-6/qml-qtquick-dialogs-dialog.html)。

### 托盘

实际后端通过 `QGuiApplication::platformName()` 检测。

- Wayland 下保留托盘图标，原生 `Platform.Menu` 延迟工厂保持未实例化。托盘右键请求会显示主窗口，并在窗口内显示“显示留声 / 退出”菜单；媒体播放操作继续由主窗口和 MPRIS 提供。
- 其他平台保留原有平台托盘菜单与播放操作。
- 新建 Wayland 配置默认关闭窗口即退出；已有 `close_to_tray` 值继续生效。用户可在设置中启用关闭到托盘。
- 关闭窗口时实时检查托盘是否可用；隐藏期间若托盘宿主失效，应用重新显示窗口。Ctrl+Q 与曲库更多菜单提供独立退出入口。
- 托盘、窗口、桌面启动器和 macOS 应用图标统一复用软件内的 `brand` 轮廓，采用透明背景和单色线条。托盘直接使用内嵌 SVG 生成像素，`icon.name` 保持空字符串；窗口图标根据系统浅/深色使用深色/浅色线条，保持与应用内独立主题设置解耦。安装后的 `.desktop` 使用带内容校验值的绝对 SVG 路径，避免桌面主题替换同名图标。

该策略避开应用自身在 Wayland 下创建 QWidget/QMenu 抓取弹窗的路径，保留明确的用户退出与恢复入口。

## Groove 图标资产

34 个功能/品牌图标重新绘制，使用 24×24 网格、1.75 单位线宽、圆角连接和统一光学安全区。原来的 `disc` 占位字形单独保留，资产目录共 35 个字形。

- 可编辑源：`scripts/generate-icons.py`
- 矢量资源：`crates/liusheng/qml/assets/icons/*.svg`
- 清单：`assets/icons/manifest.json`
- 应用 SVG、符号版、浅底/深底托盘版：`assets/app-icon/`
- PNG：16、24、32、48、64、128、256、512、1024 像素
- macOS：`Liusheng.icns`
- 总览：`docs/images/liusheng-icon-family.png`

Icon.qml 仅负责资源名、方形几何和着色。Qt Controls 的 IconImage 实现被隔离在这一处适配器内，统一 SVG 资源由 qrc 打包；当前 Qt 版本的像素解码和着色由控件测试持续验证。界面图标使用本地资源路径，移除了运行时拼装 SVG data URI 的方式。

应用窗口图标、托盘和 Linux hicolor 安装文件均已接入。Linux 安装脚本安装主 SVG、符号 SVG、内容寻址 SVG 和八档 PNG，并更新图标目录时间戳；实际安装时调用可用的桌面缓存刷新工具。内容寻址文件名随图标内容变化，`.desktop` 的绝对路径包含安装前缀，打包暂存目录保持在路径之外。卸载只删除留声的明确文件名。macOS 打包脚本和 Info.plist 接入 ICNS。

`tests/test_icon_identity.py` 校验所有品牌 SVG 的路径、线宽和 viewBox 与软件内 `brand.svg` 完全一致，逐尺寸比较 PNG 的透明通道，并核对 ICNS 内的 PNG 数据。主图标导出使用与截图一致的浅色线条；浅色系统主题提供深色线条变体。KDE 支持的 SVG 路径包含 `ColorScheme-Text` 着色标记，固定 PNG 的着色保持导出值。该标记只控制颜色；托盘采用直接像素传递，启动器采用绝对路径，不再通过主题名称选择品牌轮廓。

本轮统一图标后，在 Fedora 项目目录执行 `just install` 更新已安装的二进制与图标，使用 Ctrl+Q 结束旧进程后重新启动。仅重建二进制时，桌面启动器中的已安装图标资源仍需安装步骤同步。

## 逐页验收

`ValidationHarness.qml` 执行 50 个分步页面/交互场景，包括专辑、艺术家、歌曲、详情、歌单、队列、正在播放、设置三组、输出弹层、上下文菜单、排序下拉、歌单命名/删除确认、无效目录、托盘兼容菜单、文件与目录对话框。

每张截图只写入一次，保留弹层实际打开时的画面。独立预览脚本还生成中文、日文、英文和混合超长标签、歌词及长歌单名称，使用虚构曲库与本地生成的测试封面。生产曲库参与范围为零。

测试入口：

```sh
just ui-test
just ui-controls-test
just wayland-test
just icons-check
just ui-preview
python3 scripts/preview-ui.py target/debug/liusheng --long-labels --output target/qa/long-labels
```

`wayland-test` 启动私有 Weston headless/pixman 合成器，设置真正的 `QT_QPA_PLATFORM=wayland`，在 100%、125%、150% 缩放下运行同一套控件与应用测试。它使用独立 socket、HOME、XDG 和 D-Bus 会话，并检查平台标记、可见原生 Popup 数量和 Qt 日志；不连接已有桌面合成器。

23 项 Qt Quick 控件测试覆盖实际鼠标/键盘输入、菜单四角定位、嵌套关闭、焦点、队列拖动、长文本、图标比例、网格列数、宽屏歌词、输出长内容与主题对比度；框架另计初始化和清理。Rust workspace 含 CoreAudio 接口检查共 96 项。

## 交付验证与范围

验证日志保存在 `target/qa/polish/`，包括 release 构建、Clippy、Rust 测试、原生 Wayland 三档缩放、界面与功能回归、SVG/PNG/ICNS 校验和 PREFIX/DESTDIR 安装卸载验证。最终通过状态以 `validated-release/result.exit` 和 Wayland `results.json` 为准。

当前自动环境为 Debian 13、Qt 6.8.2、Weston headless/pixman。原生 Wayland 窗口协议已经实际运行；鼠标与键盘由 Qt 测试输入合成。Fedora/KDE 的真实托盘宿主、物理输入设备、系统文件选择门户、输入法、GPU 驱动和多显示器之间的缩放迁移，继续作为桌面验收范围。macOS 原生运行由对应 CI/实机验证，Linux 容器只校验可移植接口与 ICNS 资产。

## 旧托盘 / 启动器图标的加载优先级修复

源码中的 SVG 已一致时，仍需检查实际的图标加载路径。Qt Labs Platform 使用 `QIcon::fromTheme(icon.name, fallback)`；旧代码同时设置名称和源资源，命中主题名称后会使用主题提供的图案。桌面入口原来使用 `Icon=io.github.dhkun.Liusheng`，同样经过主题查找。

当前处理：托盘 `icon.name` 为空，通过 StatusNotifierItem 的 IconPixmap 传递内嵌品牌；启动器 `Icon=` 直接指向安装前缀下 `io.github.dhkun.Liusheng.brand-<SHA-256 前16位>.svg`。保留应用 desktop ID 和常规 hicolor 兼容资源。修改图标内容会产生新的文件名，安装后的启动器由此获得新的图像缓存键。RPM、DEB、Arch 的资源目录继续随打包暂存树携带该文件。

验证：`scripts/check-tray-icon.py` 启动独立 Weston、D-Bus 与模拟托盘宿主，并构建一个含同名红色旧图标的主题。修复前的二进制导出主题 IconName 和红色像素；修复后 IconName 为空，导出的24/22像素图与指定品牌轮廓匹配。该测试覆盖真实托盘协议传输；用户实际 KDE 面板的缓存与固定入口状态仍需在用户桌面读取。

只读诊断：`just icons-diagnose`。输出已安装 / 构建程序的 SHA-256、当前用户留声进程的实际执行文件、候选 desktop 文件的 Exec / Icon，以及与留声有关的任务栏固定项。该命令保留所有文件和进程。

安装命令显式给 Cargo 传入目标目录，安装器报告图标绝对路径、程序校验值，并在检测到同用户旧进程时提示 Ctrl+Q 后重新启动。更新程序文件后，已经运行的单实例进程仍须退出；新调用可能转交给旧实例。

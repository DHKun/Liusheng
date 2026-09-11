# 整句歌词恢复与 Wayland 托盘菜单修复

## 变更边界

用户反馈逐词高亮效果与延迟影响阅读，同时窗口最小化/隐藏后右键托盘无响应。本轮统一恢复整句高亮，并修复原生托盘菜单导出与窗口恢复。在线封面、在线歌词、已选资源、资料来源、歌词偏移、音频后端、系统媒体控制与品牌图标保持原有实现。

## 歌词

`LyricsView.qml` 恢复 Qt Quick 原生 `Text`，当前句颜色由行开始时间直接控制，颜色切换无额外缓动。逐字/逐词遮罩、普通 LRC 的近似推进、高亮模式设置、专用 C++ 渲染器与 16ms 刷新分支退出当前构建。通用播放时钟保持 33ms 插值，并保留暂停冻结、隐藏停止与实际进度快照校正。

增强 LRC、TTML 和明文 YRC/QRC 的解析与导入保留，供已导入资料继续使用，界面统一按整句显示。旧设置中的 `lyric_highlight` 不再影响界面，其余偏好与资源偏移继续恢复；保存设置时该旧字段自然移除。详见 `WORD_TIMED_LYRICS.md`。

## 托盘根因与复现

原代码在 Wayland 下设置 `menu: null`，并把右键依赖放在 `Context` 激活回调中。桌面托盘宿主可直接通过 StatusNotifierItem 的 `Menu` 对象路径读取菜单，该路径依赖已绑定的原生菜单。旧实现返回 `/NO_DBUSMENU`，支持 DBusMenu 的宿主因此得到空菜单。

本轮用私有 D-Bus 和隔离 Weston 启动实际 release 程序，旧二进制稳定返回 `/NO_DBUSMENU` 并使新增断言失败。修复后路径为 `/MenuBar`，接口提供 `com.canonical.dbusmenu.GetLayout`、`AboutToShow` 和 `Event`。

复现证据位于 `target/qa/line-lyrics-tray/reproduce-before/`。Qt 6.8.2 相关源文件保存在该目录的 `reference/` 中：`QStatusNotifierItemAdaptor::menu()` 根据是否存在菜单选择上述路径，`QDBusTrayIcon::createMenu()` 为托盘创建 `QDBusPlatformMenu`。

## 修复方式

1. 使用 `SystemTrayIcon.menu: Platform.Menu { ... }` 内联声明，在 QML 构造阶段把菜单归属到托盘，让 Qt 创建专用原生菜单。Wayland、X11 和 macOS 共用菜单定义。
2. 原生菜单包含显示、隐藏到托盘、上一首、继续/暂停、下一首和退出。菜单内容由桌面读取，隐藏状态下的读取与切歌继续保留隐藏状态。
3. 显示操作始终显式请求 `showNormal()` 或之前的最大化/全屏模式，随后在用户操作调用栈内请求激活。单独调用 `show()` 可能保留最小化状态；Wayland xdg-shell 的最小化是单向请求，窗口状态也可能继续显示 Windowed，因此恢复逻辑独立于这个状态判断。
4. 只发送 `ContextMenu` 的旧宿主继续使用窗口内 `Popup.Item` 后备菜单。先恢复窗口，等待映射，再锚定可见的窗口内容；沉浸页隐藏侧边栏时也能使用。等待有明确次数上限，所有普通应用菜单继续使用窗口内弹层。

托盘宿主规范：https://specifications.freedesktop.org/status-notifier-item/latest/status-notifier-item.html

## 验收

`scripts/check-tray-icon.py` 在私有 D-Bus、隔离 HOME/配置/曲库和 Weston 中运行实际程序，保留已确认图标的像素对照，并新增菜单行为检查。测试通过真正的 DBusMenu 点击事件验证隐藏、隐藏后读取菜单、隐藏时上一首/下一首、暂停状态保留、显示、左键激活、MPRIS Raise、旧 ContextMenu 后备路径，以及隐藏后退出和会话保存。

`scripts/check-ui.py` 验证主程序中的整句歌词、本地/导入资料、最小化恢复请求、隐藏后的沉浸页后备菜单、Esc 焦点和最大化状态恢复。Wayland 回归在 100%、125%、150% 下运行相同场景。

最小化检查按平台可观测能力验收：offscreen 后端检查明确的 Minimized 状态及恢复；Wayland 开启测试进程协议日志，要求实际发送 xdg_toplevel.set_minimized()，再验证恢复后的窗口、后备菜单和最大化状态。xdg-shell 未定义“已最小化”通知，测试保留了真实协议请求检查，避免把合成器内部状态写成跨平台断言。KDE 实际面板和物理输入留待桌面验收。

```sh
just lyric-test
python3 scripts/check-tray-icon.py target/release/liusheng --output target/qa/line-lyrics-tray/tray
python3 scripts/check-ui.py target/release/liusheng --output target/qa/line-lyrics-tray/ui
python3 scripts/check-wayland.py target/release/liusheng --output target/qa/line-lyrics-tray/wayland
```

当前完整回归结果保存在 `target/qa/line-lyrics-tray/final/`，汇总为 `target/qa/line-lyrics-tray/summary.json`。原生 macOS 和 Fedora/KDE 实机依旧按其平台验收；Linux 容器测试明确覆盖实际 Wayland 和 D-Bus 协议，不替代真实桌面的外观检查。

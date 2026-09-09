# 检查更新

## 用户操作

手动入口有两处：`设置 → 关于与更新 → 检查更新`，以及曲库右上角更多菜单中的 `检查更新…`。结果包括当前版本、最新正式版本、发布时间、发布说明和当前平台可用的安装包。

启动检查默认开启，可在 `设置 → 关于与更新` 取消勾选并保存。首帧显示、缓存曲库准备完成后等待 5 秒，再发起一次异步请求。手动检查与启动检查共用一个服务；进行中的请求会合并。自动检查失败仅记录状态，播放和窗口继续工作。

发现较新版本后，曲库顶部显示轻提示。它保留用户焦点；沉浸页和隐藏窗口暂缓展示，返回曲库时可以查看。操作区提供：

| 操作 | 行为 |
| --- | --- |
| 查看更新 | 打开窗口内的更新详情 |
| 稍后提醒 / 关闭提示 | 本次运行收起该版本提醒；下次启动检查后可再次提示 |
| 跳过此版本 | 持久化精确版本号；下一版本仍可提示 |
| 恢复提醒 | 在关于与更新中清除跳过偏好 |
| 手动检查 | 即使自动检查关闭或该版本已跳过，仍可查看结果并下载 |
| 下载更新 | 在系统浏览器中打开经过校验的官方安装包地址，由用户完成下载与安装 |

自动发现发生在每次启动；长时间保持应用开启时，可以通过手动检查发现新 Release。当前实现采用启动轮询，没有服务端常驻推送。

程序不会自动执行安装包或覆盖正在运行的二进制。DEB、RPM、AppImage 和 macOS 应用的安装流程分别由系统工具或用户管理。源码/用户目录运行时额外提示沿用原安装方式，避免安装系统 RPM 后仍从用户目录启动旧程序。

首个包含此功能的版本仍需要按原方式安装，此后才能使用应用内更新检查。已有配置、曲库数据库及音频引擎保持原有格式与职责。

## Release 契约

固定检查地址：

```text
GET https://api.github.com/repos/DHKun/Liusheng/releases/latest
```

接口使用公开读取权限和 `X-GitHub-Api-Version: 2022-11-28`，不要求个人令牌。发行者继续使用现有 `make_latest: true` 发布流程：公开的正式 Release、`vX.Y.Z` 版本号和同版本安装包是预期格式。

客户端再次核验 `draft=false`、`prerelease=false`，并对版本做数字分段比较。`0.10.0` 高于 `0.9.9`；相同数字版本的正式版高于其预发布构建；构建元数据不触发更新；旧版本与相同版本均不提示降级。

下载包按既有四目标命名识别：

```text
liusheng_X.Y.Z_amd64.deb
liusheng-X.Y.Z-1.fcNN.x86_64.rpm
liusheng-X.Y.Z-linux-x86_64.AppImage
liusheng-X.Y.Z-macos-arm64.zip
```

Linux x86_64 显示 DEB、RPM、AppImage；macOS arm64 显示 Apple Silicon ZIP。AppImage 运行优先推荐 AppImage，Fedora 系优先 RPM，Debian/Ubuntu 系优先 DEB；这是格式建议，实际系统版本要求以新 Release 说明为准。未知架构、缺失包、未完成上传的包、同类型重复包或无效下载地址会降级为发布页入口，界面保留解释。

目前读取 latest 单个 Release；Git 标签推送本身和草稿 Release 不构成客户端更新。

## 工程实现

| 文件 | 职责 |
| --- | --- |
| `src/updates/release_policy.h` | 纯版本解析、比较、正式发布与资产/URL 校验 |
| `src/update_service.h/.cpp` | Qt Network 异步请求、超时、缓存、状态、浏览器跳转 |
| `qml/UpdateDialog.qml` | 手动检查、结果、说明、包选择与用户操作 |
| `qml/UpdateNotice.qml` | 不争夺焦点的轻提示 |
| `qml/Main.qml` | 首帧后调度、服务实例和惰性更新对话框 |
| `liusheng-core/src/settings.rs` | `check_updates_on_startup` 偏好，默认值兼容旧设置 |

Qt Network 已在原项目中使用，本轮复用既有依赖。更新状态独立于播放器和曲库服务；联网失败不会写入音频错误，也不会触发曲库重新扫描。

自动更新开关通过现有设置存储。跳过偏好存入独立 `update-preferences.json`，避免打开的设置草稿覆盖刚更新的跳过版本。

```text
${XDG_CONFIG_HOME:-~/.config}/liusheng/update-preferences.json
${XDG_CACHE_HOME:-~/.cache}/liusheng/release-cache.json
```

写入使用 QSaveFile 原子替换及仅用户读写权限。ETag 与 Release 缓存经同一套校验后才使用；304 响应复用已校验数据。缓存损坏时重新请求；无缓存的意外 304 最多补发一次无条件请求。失败时保留最近成功检查时间，并明确显示失败状态。

## 网络与安全边界

- 生产端点固定为 GitHub API，TLS 使用系统证书验证和系统代理设置。响应重定向会停止检查并提示前往发布页。
- 传输空闲上限 8 秒，单次请求绝对期限 10 秒，响应最大 256 KiB，界面说明最长 24,000 字符。
- 尊重 403/429 的 Retry-After / X-RateLimit-Reset，等待期上限一天，持久化至下次启动；连续手动点击有 3 秒间隔。
- 客户端检查下载网址的 HTTPS、主机、仓库、版本、文件名、用户信息、端口、查询和片段。所有 URL 跳转都需要用户操作。
- 说明使用 Text.PlainText，远程 HTML、图片和脚本保留为文字；应用不加载发布说明中的远程内容。
- 请求不附带账号令牌、Cookie、曲库、音乐路径或播放记录。GitHub 仍会接收正常网络连接信息和应用 User-Agent，用户可关闭启动检查。
- AppImage 必须携带 Qt OpenSSL TLS 插件以及匹配的 libssl/libcrypto；系统 CA 证书和系统代理仍由宿主提供。

`--no-update-check` 可以为本次运行完全关闭更新联网；QA、基准、UI、功能和输出测试会自动阻止公开更新请求。测试服务器注入只在独立 C++ 测试宏下编译，生产程序提供固定端点。

## 测试

```sh
just updates-test
python3 scripts/check-controls.py
python3 scripts/check-ui.py target/release/liusheng --output target/qa/updates/ui
python3 scripts/check-wayland.py target/release/liusheng --output target/qa/updates/wayland
```

`updates-test` 通过 qmake 编译实际生产 UpdateService，以本地 TCP/HTTP 服务器覆盖版本、网络、缓存和状态迁移。所有配置均在临时目录，浏览器跳转被测试实现记录，不会自动下载软件。可额外执行一次真实 HTTPS 元数据检查：

```sh
python3 scripts/check-updates.py --live
```

普通 Linux、macOS arm64 CI 与发布工作流均运行隔离测试。界面回归覆盖新入口、关于页面、勾选保存/取消、更新对话框层级、Esc 与窄窗口；AppImage 界面测试确认打包后 TLS 提供者可用。执行脚本使用 Python 3.11+，与项目其他打包测试保持一致。

实际验证日志集中在 `target/qa/updates/final/`。Wayland 使用独立 Weston 和合成 Qt 输入；KDE 桌面门户、浏览器唤起以及原生 macOS 的最终体验继续通过实机与各平台 CI 验收。

## 官方参考

- GitHub Releases API：<https://docs.github.com/en/rest/releases/releases#get-the-latest-release>
- GitHub 条件请求和限流处理：<https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api>
- Qt 异步网络访问：<https://doc.qt.io/qt-6/qnetworkaccessmanager.html>
- Qt 请求重定向与超时：<https://doc.qt.io/qt-6/qnetworkrequest.html>
- Semantic Versioning：<https://semver.org/>

## 本轮验证记录

Linux 工作区 136 项 Rust 测试、65 项 Python 回归和 42 项 QML 用例通过。更新策略/传输共 37 个测试场景分别使用 GCC 和 Clang 编译运行通过（Qt 测试框架另计初始化/清理两项），并执行了一次真实 GitHub HTTPS 元数据请求，识别到 v0.3.3 及三个 Linux 安装包。

Release 的页面/功能、单实例/MPRIS 回归，以及 Weston 100%/125%/150% 均通过。实际 AppImage 完成打包、带空格路径迁移、自解包启动、完整界面及 TLS 提供者检查。原生 macOS 的网络/策略测试已接入 arm64 CI，最终结果待推送执行。详见 target/qa/updates/final/summary.json。

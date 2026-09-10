# 留声架构图 · 源码依据

> 本索引记录其标注提交的历史行号。当前迭代以函数符号定位源码，风险处置与验证基线见 [OPTIMIZATION_ITERATION.md](../OPTIMIZATION_ITERATION.md)。

对应源码快照 `2d5ddb2`，应用版本 `0.3.0`。实际项目目录为 `/data/Project/Liusheng`。架构内容依据当前实现与项目设计文档，图形由本地 Draw.io MCP 导入、排版后导出。

主文件：`liusheng-architecture.drawio`，包含五个可编辑页面。`index.html` 内嵌五张 SVG，可离线浏览；`arch-01` 至 `arch-05` 各自提供 Draw.io、SVG、PNG。

## 阅读约定

- 总览按职责分层；第二页说明实际线程边界。一个职责框可包含相关模块或多个工作线程。
- 实线表示调用、处理结果前进或数据写入；虚线表示异步事件与结果返回。第二页画出 LibraryService 的回送通道作为共同约定示例，其余服务按页脚说明回到 GUI。
- 数据页中的 watcher、标签提取与库服务表达处理阶段。Library / SQLite 位于 LibraryService 工作线程，文件监听拥有独立事件合并线程。
- 音频页同时显示三个可选后端。运行时依据平台与输出模式选择一个后端；预加载结果交回 Player。重采样仅在 44.1 kHz 原生输出不受设备支持时进入既有 96 kHz / 24-bit 适配路径。
- 源码配置与历史实机验收分别记录。图中的 CI、后端与发布产物表示当前工程实现及配置。

## 实现索引

以下路径相对项目根目录，行号用于快速定位。

| 图中内容 | 代码位置 | 关键符号或事实 |
| --- | --- | --- |
| 工作区与版本 | `Cargo.toml:1`；`crates/liusheng/Cargo.toml:1`；`crates/liusheng-core/Cargo.toml:1` | 两个 crate、Rust 2024、Qt / 平台依赖 |
| 应用启动 | `crates/liusheng/src/main.rs:17` | `main`、单实例检查、加载 Main.qml |
| 桌面文件入口 | `crates/liusheng/src/desktop_bridge.h:37`、`:149` | QLocalServer、QLockFile、用户级 IPC |
| 项目视觉令牌 | `crates/liusheng/qml/Theme.qml:11` | 暖白 `#faf9f6`、炭灰 `#242522`、棕色 `#885b38` |
| Qt 桥接与模型 | `crates/liusheng/src/app_controller.rs:33`；`crates/liusheng/src/models.rs:120` | `AppControllerRust`、`UiModelRust` |
| 后台服务接入 | `crates/liusheng/src/app_controller/services.rs:4` | `start_services`、事件接收线程、`qt_thread.queue` |
| 输出会话延迟创建 | `crates/liusheng/src/app_controller.rs:1221` | `ensure_output_session` |
| 异步歌词 | `crates/liusheng/src/app_controller.rs:1324` | `request_lyrics_for_path`、当前路径校验 |
| 曲库工作线程 | `crates/liusheng-core/src/library/service.rs:59`、`:187` | 请求容量 32、缓存 Ready 后后台校验 |
| 文件监听 | `crates/liusheng-core/src/library/watcher.rs:37`、`:177` | `LibraryWatcher`、`run_event_loop` |
| 搜索工作线程 | `crates/liusheng/src/search_service.rs:24` | 最新请求槽、wake 容量 1、generation 校验 |
| 封面工作线程 | `crates/liusheng-core/src/artwork_service.rs:54` | 请求容量 128、缩略图与版本 manifest |
| 硬件音量 | `crates/liusheng/src/volume_service.rs:14`；`crates/liusheng-core/src/audio/hardware_volume.rs:29` | 请求容量 16、ALSA mixer |
| MPRIS | `crates/liusheng/src/mpris.rs:523` | 独立服务线程、播放快照与 D-Bus 命令 |
| 输出模式与恢复 | `crates/liusheng-core/src/output_session.rs:39`、`:110` | `SessionEvent`、平台后端工厂、独占重试 |
| 播放引擎 | `crates/liusheng-core/src/engine.rs:71`、`:190` | `Player`、引擎线程与播放循环 |
| 预加载 | `crates/liusheng-core/src/engine/preload.rs:28` | `Preloader::new`，约 250 ms 首批 PCM |
| 解码 | `crates/liusheng-core/src/audio/decode.rs:16` | `AudioFileDecoder`，交错 i32 + `PcmSpec` |
| 后端与重采样 | `crates/liusheng-core/src/audio/resampling_sink.rs:72`；`crates/liusheng-core/src/audio/pipewire_sink.rs:459` | 原生能力判断、PipeWire 回调 |
| 数据表 | `crates/liusheng-core/src/library/db.rs:58`、`:83`；`crates/liusheng-core/src/library/playlists.rs:13` | tracks、WAL、schema v2、路径型歌单项 |
| 设置与会话 | `crates/liusheng-core/src/settings.rs:11`、`:103`、`:178` | AppSettings、SavedSession、AppPaths |
| 验证与发布 | `.github/workflows/check.yml:1`；`.github/workflows/release.yml:1` | Linux / macOS CI、标签发布与安装包 |

## 设计文档

主流程同时核对了 `README.md`、`DECISIONS.md`、`CONTEXT.md`、`docs/UPGRADE_0_3.md` 与 `docs/GUI_DESIGN.md`。其中历史路线图与当前实现并存；图形采用 0.3.0 当前实现，CUE、联网歌词、FTS5 等待评估能力保留在原文档中。

## 检查记录

五页均通过原生 XML 解析、页面内 ID 唯一性、边端点与父节点引用检查；逐页检查 MCP 导出的 PNG，调整了跨区连线、预加载回送方向与 MPRIS 发布来源。此次只新增架构文档与图片。

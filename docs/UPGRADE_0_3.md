# Liusheng 0.3.0：功能与性能升级记录

基线：`fed28d5`（0.2.1）。日期：2026-09-07。此次修改保留 Rust 核心、Qt Quick/QML、cxx-qt 和既有本地音频定位。

## 已实现的主要变化

| 领域 | 实现 |
| --- | --- |
| 启动 | SQLite 缓存曲库先发布，首次导入分批发布，目录扫描及递归监听在后台建立；音频输出延迟至播放请求 |
| 扫描 | mtime + 文件长度校验、128 条元数据批次、按路径变化更新、目录范围隔离、失败子树及离线目录保护、错误明细、取消检查 |
| 搜索 | 复用持久化拼音字段、独立搜索线程、最新请求覆盖、过期结果丢弃、排序与格式筛选 |
| 数据模型 | QAbstractListModel 角色、稳定条目标识、结构变化通知、元数据 dataChanged；歌曲元数据使用 Arc 共享 |
| 封面 | 有界任务队列、可见页面请求、256/768 像素缩略图、持久化版本索引、主题色缓存、缺失缓存与批量旧版本清理 |
| 播放 | 独立下一曲预加载、手动切歌复用、随机/单曲循环/列表循环、队列拖动和按钮排序、暂停切歌保持状态 |
| 音频后端 | PipeWire 和 CPAL 回调采用预分配 SPSC 缓冲；写入支持取消与超时；ALSA 使用非阻塞写入、错误分类重试和原生格式校验 |
| 设备 | ALSA 输出枚举与手动配置、混音器配置、变更输出设备时事务恢复、44.1 kHz 原生能力探测；N9 继续可用 44.1→96 kHz 适配 |
| 歌单 | SQLite 持久化、重复曲目及顺序保留、保存队列、改名/删除、M3U8 导入导出 |
| 会话 | 保存队列、当前条目、位置、随机/循环状态、窗口尺寸、页面及列表位置；启动恢复为暂停；暂停恢复期间可定位及切歌 |
| 桌面集成 | 文件选择、拖入、命令行、MPRIS OpenUri、用户级单实例 IPC、macOS 文件打开事件与文件关联 |
| 歌词与界面 | 每曲偏移、LRC 修改刷新、封面提色、平滑显示时钟、沉浸页/设置/歌单页面延迟创建、输出信号链面板 |
| 交付 | 0.3.0 版本号、运行依赖、CARGO_TARGET_DIR 安装兼容、PR 质量检查、Linux 离屏回归和原生 macOS CI 任务 |

## 实现结构

```text
Qt Quick 页面 / UiModel
          ↕
AppController（Qt 桥接及交互协调）
          ├── LibraryService：SQLite、扫描、监听、歌单、设置、会话写入
          ├── SearchService：持久化检索字段、排序、格式筛选、过期请求控制
          ├── ArtworkService：按需缩略图、提色、版本缓存、批量清理
          ├── VolumeService / MPRIS：平台 I/O 工作线程
          └── OutputSession → Player → AudioSink
                                  ├── Preloader：下一曲打开与预解码
                                  └── SPSC → PipeWire / CPAL 输出回调
```

主要文件：`library/service.rs`、`library/playlists.rs`、`settings.rs`、`artwork_service.rs`、`engine/preload.rs`、`queue_order.rs`、`models.rs`、`app_controller/services.rs`、`desktop_bridge.h`。

Qt 模型仅在 GUI 线程提交修改，后台服务通过不可变快照交接数据。元数据共享降低队列复制成本；会话中的队列路径同样使用共享快照，并按队列版本更新，常规进度保存只替换小型状态。MPRIS 连接与属性发布由独立线程处理。

## 数据迁移和升级使用

既有曲库路径保留：Linux 默认 `~/.local/share/liusheng/library.db`；XDG 环境变量继续生效。为兼容既有版本，macOS 的旧数据路径也保持原状。

SQLite schema v2 增加 `file_size` 与艺术家索引，复用原有搜索列，增加独立的歌单与歌单条目表。旧库迁移保留歌曲与搜索字段，文件长度未知的旧条目在下一次校验补齐。遇到较新数据库版本会报告错误并保留该版本号。

新文件：

- 设置：`$XDG_CONFIG_HOME/liusheng/settings.json`，默认 `~/.config/liusheng/settings.json`。
- 会话：`$XDG_DATA_HOME/liusheng/session.json`。
- 封面：`$XDG_CACHE_HOME/liusheng/covers/`，含缩略图及 `thumbnails-v1.json`。

设置、会话和生成缓存使用临时文件加原子替换。升级真实曲库前，先退出旧版并备份整个 Liusheng 数据目录；退出期间保留 SQLite 主库与可能存在的 WAL 文件一起备份。

在项目目录运行：

```sh
cargo build --release --locked -p liusheng
./target/release/liusheng
# 验收后安装到 ~/.local
just install
```

使用自定义 `CARGO_TARGET_DIR` 时，运行对应目录中的二进制；安装脚本同步遵守该设置。容器构建位于 `/tmp/liusheng-target`，不会替换宿主机已安装应用。

排除目录会停止扫描和自动更新对应文件，并保留既有缓存记录；移除根目录同样保留已有曲库和歌单引用，便于重新挂载及恢复。

## 本次实际验证

测试环境为 Debian 13 开发容器、Rust 1.98.1、Qt 6、离屏软件渲染。测试数据全部在独立临时 HOME/XDG 目录中生成。

| 检查 | 结果 |
| --- | --- |
| 原始版本核心测试 | 61 项通过 |
| 0.3.0 workspace + Linux CPAL 合约测试 | 93 项通过：UI/模型 10、core 单元 59、解码 4、引擎 13、曲库集成 7 |
| `cargo clippy ... -- -D warnings` | 通过 |
| `cargo fmt --all --check` | 通过 |
| `cargo build --release -p liusheng` | 通过，容器 Linux 二进制约 19 MB |
| 所有主要页面与延迟页面加载 | debug 和 release 离屏检查通过，完成截图检查 |
| 会话与交互 | 暂停恢复、恢复后定位/切歌、移动和删除当前条目、循环/随机设置通过 |
| 歌单与搜索 | 保存/改名/删除、M3U8 往返、搜索筛选和设置写入通过 |
| 桌面集成 | 同用户单实例唤起、MPRIS 暂停态控制和优雅退出保存通过 |
| 音频运行 smoke | 隔离 PipeWire + WirePlumber + 空输出节点，播放、暂停/继续、队列结束和退出通过 |
| 安装脚本 | 自定义 target 目录与临时 DESTDIR 安装通过 |

新增回归覆盖旧数据库迁移、未来版本保护、多根目录隔离、离线保护、按路径更新、扫描取消、有界写入取消、循环播放、设备切换失败恢复、SPSC 完整帧/环绕/补零、歌单重复条目和特殊字符文件名。

复现命令：

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --features liusheng-core/coreaudio-compile-check --locked -- -D warnings
cargo test --workspace --features liusheng-core/coreaudio-compile-check --locked
cargo build --release --locked -p liusheng
python3 scripts/check-ui.py target/release/liusheng --output target/qa
python3 scripts/benchmark-startup.py target/release/liusheng --runs 7 --output target/qa/startup-benchmark.json
```

`check-ui.py` 在独立 D-Bus 会话中运行，使用临时音乐和配置；测试结束清理临时数据。变更数据的 `--functional-test` 需要专用目录标记和匹配的 XDG_DATA_HOME。`--ui-test` 用于页面遍历；日常启动使用普通命令。

## 启动基准结果

条件：release、Qt offscreen/software、已预热缓存、合成 SQLite 元数据、音乐根目录离线，每组 7 次取中位数。首帧和可交互标记从 DesktopBridge 创建时开始计时；数据库快照计时从 LibraryService 初始化开始。进程总时间还包含 D-Bus 会话启动与工作线程退出。

| 合成曲库 | 数据库缓存快照 | 首帧标记 | 界面可交互标记 | 进程总时间 |
| --- | ---: | ---: | ---: | ---: |
| 1,000 首 / 100 张专辑 | 6 ms | 27.21 ms | 68.36 ms | 398.94 ms |
| 10,000 首 / 1,000 张专辑 | 37 ms | 27.55 ms | 104.31 ms | 437.38 ms |

原始采样保存在 `target/qa/startup-benchmark.json`。这些数值用于检查缓存启动关键路径；Fedora/KDE/Wayland 的窗口创建、显示驱动、真实目录和首次封面加载须单独测量。本次缺少同条件修改前启动计时，因此保持原始测量值，不推导提速倍数。

## 验证边界与后续验收

原生 macOS CI 已配置，当前容器完成的是 CPAL 接口编译和回调逻辑测试。macOS CoreAudio、Finder 文件关联、托盘行为，以及 Fedora/KDE/Wayland 下的窗口激活，需要对应系统实际运行。N9/DAC 的独占格式、硬件音量、热插拔与真实听感也需要实机测试；共享输出面板明确区分应用格式和系统最终设备格式。

文件系统读操作仍受操作系统及挂载实现影响；取消在扫描文件之间和解码块之间检查，挂载层永久阻塞需要存储层超时策略。随机模式保存开关和当前曲，重启后重新建立随机顺序。移动硬盘和移除目录保留缓存，应用显式播放文件时检查其可用性。

搜索本轮采用已缓存文本的后台过滤；FTS5、CUE、联网歌词和额外音效继续按实际曲库需求独立评估。已有原文、拼音、首字母搜索、原始整数 PCM 与本地音乐定位继续保留。

建议在 Fedora 上先使用真实曲库和 N9 完成一次“启动、播放、暂停、切歌、独占切换、设备拔插、退出再恢复”的验收，再执行安装或发布。

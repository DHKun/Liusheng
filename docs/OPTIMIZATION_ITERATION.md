# 播放一致性、增量曲库与发布可靠性迭代

基线：`75f8db8`（0.3.4）及本轮增量。原始认知报告基线为 `92879b5` 加当时工作树。本文记录当前代码、针对性回归和实际测量范围；原始 `docs/codebase-report/` 保留其历史内容。

源码版本与现有标签保持原样。本轮功能随新的源码提交进入主线；正式发布时另行更新版本并验证标签。

## 1. 已落实的改动

### 查询与歌词

组合查询使用 `query_terms → normalize_query → matches_terms` 共用语义：先按空白分词，再对每个词归一化，各词跨标题、艺术家、专辑同时匹配。Qt 专辑／艺术家模型、异步歌曲搜索和 SQLite 查询采用同一约定。清空过滤会使旧搜索 generation 失效。

歌词读取支持 UTF-8 与带 BOM 的 UTF-16 LE/BE。外部 LRC 读取、编码或大小异常后继续尝试内嵌歌词；成功回退记录来源警告，全部来源失败返回诊断。文本上限 1 MiB、10,000 行、每行 8,192 字节，多时间戳展开上限 2 MiB。原歌词时间戳、同时间主附文组合及播放定位语义保持。

### 播放顺序与输出交接

`PlaybackOrder` 以共享不可变排列保存实际播放顺序。队列插入、删除和移动通过位置映射保留既定随机历史，重复文件仍对应不同队列位置。单独修改循环模式保持随机排列。

`PlaybackCheckpoint` 由引擎在 FIFO 交接屏障捕获：此前命令处理完成后、输出缓冲丢弃前，记录队列、实际随机顺序、当前项、播放／暂停及实际位置。输出会话从该快照恢复；旧的线性 Next/Prev 预估状态已经移除。

`NavigationState` 提供下一首、上一首和能力判断。GUI、Linux MPRIS 与 macOS MediaPlayer 共用这些结果，原生 macOS Previous/Next 的 enabled 状态和命令路由也检查对应能力。暂停恢复会话保存可选 `playback_order`；旧 session v1 使用默认值兼容，非法排列按队列重新生成。

解码器定位返回的实际位置可能对齐解码包边界。回归检查实际位置的保留，与用户请求的原始秒数分别记录。

### 文件扫描、取消与数据保存

`library/scan.rs` 负责只读遍历与标签准备，`scan_worker.rs` 使用单个 I/O 线程和有界消息通道（请求 1、结果 4），按最多 128 项交还数据。SQLite 继续由 LibraryService 线程独占写入；文件读取期间仍能处理设置、歌单和会话保存。

单文件事件按唯一路径索引查询；目录事件合并为最小受影响子树。范围查询使用带路径分隔符的 BINARY 上下界，中文、百分号、下划线与前缀相似目录保持各自范围。通知丢失或超限时继续允许完整核对。

取消信号贯穿遍历、准备与事务提交。删除需要成功核对的范围、仍在线的根和匹配的旧数据库状态；离线、失败和排除子树保留记录。旧扫描结果采用期望状态比较，避免覆盖较新的导入。扫描菜单在运行中显示“取消扫描”。

会话保存期限在每轮消息处理时检查，持续消息流同样推进保存；保存失败的最新检查点保留待重试。底层文件系统调用的结束时机由操作系统决定。应用退出停止接收其结果，纯只读扫描线程最多等待 100ms 后可以独立结束，其持有零数据库写权限。受控阻塞测试验证服务退出在 1 秒内完成、迟到结果保持曲库原状态。

### 快照与界面更新

相同数据快照直接复用 Arc；小量变更只重新读取变化曲目，保留其他 TrackRow 身份，再在后台调整索引。专辑／艺术家聚合和排序仍有随库规模增长的工作，本轮重点减少重复读取、字符串复制及 GUI 线程计算。

专辑／艺术家拼音摘要在曲库线程准备并按名称复用。Qt 模型共享行对象，内容相同的发布直接复用原快照；封面完成只更新关联专辑和艺术家行。无数据变化的扫描只发送完成状态。Qt begin/end 行变更、dataChanged 和界面属性继续在 GUI 线程提交。

空间动效使用 `Theme.spatial`：标准 260ms，减少动画为 0ms。图标、封面生成、音频解码、PCM 输出和重采样实现已经与基线逐字节对比，保持原样。

### 数据与发布

每个 SQLite 连接显式启用外键；schema、补列和 user_version 更新置于同一事务。回归包含仓库 `v0.1.0` 的真实 schema，验证标签、组合搜索、file_size 默认值和完整性。

普通 CI 与标签发布调用相同的 `quality.yml`，该本地 reusable workflow 使用调用方对应提交。Linux Rust／Qt／Wayland／托盘／安装合同与 macOS arm64 原生质量检查共用定义。AppImage 继续共用既有构建及 clean-runtime 工作流。

标签发布增加 DEB、RPM 的全新发行版运行容器：安装实际附件后检查版本、程序、启动器、内容寻址图标和初始 QML。macOS ZIP 解包迁移后验证架构、签名与实际应用启动。发布 job 等待这些门禁完成，仓库写权限限定为最终发布步骤。产物继续为 DEB、RPM、AppImage、macOS arm64 ZIP 和 SHA256SUMS。

卸载在普通 KDE 用户安装场景刷新 kbuildsycoca6，DESTDIR 打包场景保持隔离。

## 2. 复现与确定性回归

修改前的独立源码归档中，以下三个新断言全部失败：

| 场景 | 修改前 | 修改后 |
|---|---|---|
| “周杰伦 晴天”跨字段搜索 | 0 个结果 | 正确命中 |
| 非法 UTF-8 LRC + 有效内嵌歌词 | 返回读取错误 | 使用内嵌歌词 |
| 专辑子目录新增一曲 | 同时检查前缀相邻专辑 | 只核对目标子树 |

进一步回归包括：随机顺序与重复条目的交接、实际寻址位置、输出切换失败恢复、循环模式、队列编辑、暂停会话兼容、UTF-16、歌词输入限额、离线和删除子树、取消保数据、SQLite 外键及历史迁移、后台阻塞时的设置／歌单／会话保存、迟到结果、Qt 模型行复用和发布失败处理。

在 10,001 条缓存记录的用例中，一条文件修改取得 `visited=1`、`state_rows=1`；未修改曲目 Arc 身份保留。该计数来自实际扫描与状态读取的工作量，时间基准另行测量。

入口：

```bash
just optimization-test
cargo test --workspace --features liusheng-core/coreaudio-compile-check --locked
python3 -m unittest discover -s tests -p 'test_optimization_delivery.py' -v
```

## 3. 启动测量

测量使用同一容器的 release 程序、Qt offscreen/software、合成缓存元数据、离线音乐根和预热后的文件／字体缓存。每组 5 次取中位数。`interactive_ms` 从 DesktopBridge 创建开始，衡量曲库可交互；完整进程时长另外包含 D-Bus 启动及测试退出。

<!-- STARTUP_RESULTS -->
| 缓存曲目数 | 修改前可交互中位数 | 修改后可交互中位数 | 变化 |
|---|---:|---:|---:|
| 1,000 | 39.29 ms | 39.27 ms | -0.05% |
| 10,000 | 74.54 ms | 68.92 ms | -7.54% |
| 100,000 | 469.43 ms | 369.64 ms | -21.26% |

1,000 首接近持平；10,000 首可交互中位数降低约 7.5%；100,000 首降低约 21.3%。首帧中位数保持约 22–24 ms。100,000 首的后台缓存准备中位数由 299 ms 到 315 ms，界面可交互阶段从共享与后台预处理获得收益。时间含义及条件均保留，真实冷启动另行测量。
<!-- END_STARTUP_RESULTS -->

`benchmark-startup.py` 增加可比较条件检查和可选预算。不同后端、数据集或计时起点会拒绝混合比较；墙钟预算由调用者显式启用，确定性的扫描范围／状态读取／快照复用断言持续进入 CI。

```bash
python3 scripts/benchmark-startup.py target/release/liusheng \
  --runs 5 --sizes 1000 10000 100000 \
  --baseline target/qa/debt-iteration/before-startup.json \
  --output target/qa/debt-iteration/compared-startup.json

# 同设备、同环境的明确预算示例：允许可交互中位数最多增加 20%。
python3 scripts/benchmark-startup.py target/release/liusheng \
  --runs 5 --sizes 1000 10000 100000 \
  --baseline target/qa/debt-iteration/before-startup.json \
  --max-regression 0.20 --output target/qa/debt-iteration/budget.json
```

该基准用于比较当前实现。真实冷启动、物理声卡可听延迟、GPU 帧时间及耗电由桌面实机验收。

## 4. 当前验收范围

<!-- VALIDATION_RESULTS -->
| 验证项目 | 当前本地结果 |
|---|---|
| Rust workspace（含 CoreAudio 接口检查） | 160 项通过，新增 24 项 |
| Python 合同与脚本回归 | 77 项通过，新增 12 项 |
| QML 控件与聆听／更新界面 | 42 个用例通过（Qt 汇总 48 含初始化与清理） |
| 更新策略与异步网络 | 37 个场景通过（Qt 汇总 39 含初始化与清理） |
| Release 页面、功能、单实例与 MPRIS | 通过 |
| Weston Wayland 100%／125%／150% | 全部通过 |
| 冲突主题下托盘图标、生成资源一致性 | 通过 |
| 原生监听器／曲库五轮回归 | 全部通过 |
| Release 构建、严格 Clippy、格式、4 份 actionlint | 通过 |
| Apple arm64 核心库／示例／测试类型检查 | 通过 |
| Apple arm64 媒体适配器 Rust 严格检查 | 通过，原生 Objective-C++ 运行待 CI |
| 实际 DEB + 声明依赖的独立根验证 | 程序、启动器、图标与 QML 启动通过 |
| 实际 AppImage | 重定位、自解包启动、完整 UI 与 Wayland 三档通过 |
| AppImage 独立运行环境 | 无系统 Qt/PipeWire，128 个程序／插件依赖与启动通过 |

AppImage 的打包工具会修改 ELF 装载信息；其提取程序与原始 release 程序的 `.text` 节逐字节一致。最终 AppImage、DEB、独立运行日志与哈希见 `target/qa/debt-iteration/summary.json`。
<!-- END_VALIDATION_RESULTS -->

macOS Rust 适配器另在真实 `aarch64-apple-darwin` 目标进行独立严格 Clippy／类型检查。Objective-C++ MediaPlayer 编译和运行、实际 macOS ZIP 迁移检查，由推送后的原生 arm64 CI 执行。Fedora RPM 的 dnf 安装场景同样由其原生容器执行。

本地 DEB 验证从实际附件及声明依赖解包到隔离根，检查启动器、图标、动态依赖和 Qt 启动；CI 的 apt 安装额外执行包管理器流程。AppImage 已有实际镜像／自解包／迁移路径／Wayland 验证；无系统 Qt/PipeWire 的独立根验证提取后的程序和动态插件。

KDE 实际托盘宿主、跨屏缩放、物理 DAC、耳机媒体键、挂载文件系统失联、macOS 控制中心和睡眠唤醒保持实机验收。Developer ID 与公证需要维护者账号凭据；现有 ad-hoc 签名继续保留。跨格式无缝衔接、联网歌词、文件重命名后的歌单修复属于单独的后续产品工作。

## 5. 原始报告 35 项处置

下表序号对应原始 `data/findings.json` 的数组顺序。原严重级别保留供追溯；当前处置区分缺陷、设计约束、已提交能力、历史文档与后续产品能力。报告页面的 HTML 冒烟结果单独说明报告可用性；应用质量引用实际程序验证。

<!-- FINDINGS_RESULTS -->
| 编号 | 原发现 | 当前处置 | 说明 |
|---|---|---|---|
| R01 | 声音路径无 EQ/ReplayGain/软件音量 | 保留设计约束 | 音频后端、解码和重采样逐字节保持；播放调度独立改进。 |
| R02 | Gapless 只保证相同 PcmSpec 的样本拼接 | 保留已知边界 | 同格式自动衔接维持既有样本验证；跨格式重配置作为独立后续工作。 |
| R03 | PlaybackResume 的 Next/Prev 按线性 index，忽略 shuffle/repeat | 已修复 | PlaybackCheckpoint FIFO 交接取代引擎外的线性 Next/Prev 预估；保留实际随机顺序。 |
| R04 | 预加载未完成时 EOF 忙等 1ms，可能产生微间隙 | 已修复 | EOF 等待改为命令／预加载通道选择，停止每毫秒轮询。 |
| R05 | macOS 共享输出在回调中转为设备格式，非 bit-perfect | 保留平台范围 | macOS 保持 CoreAudio 共享输出与系统设备适配。 |
| R06 | 独占 EBUSY 仅重试 errno 16，未调用 WirePlumber API | 保留操作边界 | ALSA 占用重试维持原策略，真实设备协调进入硬件验收。 |
| R07 | preload_next 声明返回坏文件列表但恒为空 Vec | 已清理 | preload_next 返回 ()，错误统一从异步预加载结果发布。 |
| R08 | GUI 搜索不走 SQLite Library::search | 统一行为并保留架构 | GUI、模型和 SQLite 共用组合关键词语义；索引选型依测量安排。 |
| R09 | 目录监听的 scoped scan 实际是整根重扫 | 已修复 | 最小子树核对及索引路径查询；万级缓存单文件用例 visited=1/state_rows=1。 |
| R10 | playlist FOREIGN KEY 未启用 | 显式加固 | 每连接 foreign_keys=ON，并验证孤立歌单条目被拒绝。 |
| R11 | upsert_track 不写 file_size | 保留事务契约 | Prepared 批次内 upsert 与 file_size 写入共享事务，旧状态比较保护新导入。 |
| R12 | Linux 默认音乐根依赖 /data/Music 是否存在 | 文档按实际默认值说明 | 保留 /data/Music 存在时优先、否则用户 Music 的既有行为。 |
| R13 | 联网抓词未实现 | 后续产品能力 | 本轮完善本地歌词来源与回退，联网抓词另行设计。 |
| R14 | schema 迁移只补 file_size 不补检索列 | 历史范围已验证 | 加入 v0.1.0 实际 schema 迁移夹具及搜索／完整性验证。 |
| R15 | sidecar LRC 读失败不回退内嵌歌词 | 已修复 | 异常外部歌词回退内嵌，UTF-16 与资源限额同步覆盖。 |
| R16 | 更新检查是工作树草稿，不是已发布能力 | 已由基线提交关闭 | 75f8db8 已包含更新实现、构建入口与测试。 |
| R17 | MPRIS CanGoNext 不考虑列表循环/随机，macOS canNext 考虑 | 已修复 | MPRIS／macOS 读取相同导航能力；原生 Previous/Next 回调也校验该能力。 |
| R18 | 二次实例在锁冲突且连不上已有套接字时直接退出 | 保留安全退出策略 | 锁冲突且 IPC 失败时保持单实例约束；异常启动体验列为后续实机排查。 |
| R19 | 架构索引行号相对当前工作树部分过期 | 历史索引已标注 | SOURCES 保留对应提交行号，当前通过函数符号与本迭代记录定位。 |
| R20 | GUI_DESIGN 写设置三个分组，源码已有第四个『关于与更新』 | 文档已更新 | GUI_DESIGN 设置分组更新为四组，包含关于与更新。 |
| R21 | UpdateDialog.qml / UpdateNotice.qml 尚未 git 跟踪 | 已由基线提交关闭 | 两个更新 QML 文件在 75f8db8 中已跟踪。 |
| R22 | CoverFlight/歌词滚动/listeningProgress 使用 260ms，Theme.slow 为 200ms | 已统一 | 260ms 空间动效集中到 Theme.spatial，减少动画值为 0。 |
| R23 | LyricsView 整表 JSON.parse(lyricCuesJson)，未用 lyricText/lyricTimeMs invokable | 保留并设置边界 | 歌词变化时解析 JSON；新增字节、行数、单行和时间戳展开限额。 |
| R24 | Wayland 窗口内托盘菜单只有显示/退出，不含播放控制 | 保留 Wayland 策略 | 应用内 Popup.Item 与 MPRIS 保持，托盘兼容菜单继续使用安全入口。 |
| R25 | SettingsDialog.validPaths 只接受以 / 开头的路径 | 保留平台路径契约 | Linux／macOS 音乐目录继续要求绝对路径。 |
| R26 | assets/tray.svg 未被 Main.qml 引用 | 用途已确认并保留 | tray.svg 由 generate-icons.py 生成，test_icon_identity.py 纳入一致性检查。 |
| R27 | UI_POLISH 写 ValidationHarness 50 步，源码 ui-test 已到 case 66+更新场景 | 历史结果已标注 | UI_POLISH 测试数字保留历史基线；当前验证以测试执行记录为准。 |
| R28 | 播放栏右侧按钮区固定 width=124，窄窗可能挤压中部控件 | 已回归验证 | 正常／紧凑窗口、浅深主题和 Wayland 三档缩放通过实际界面回归。 |
| R29 | 文档仍写准备版本 0.3.2，crate/lock 已是 0.3.3 | 文档已更新 | 现行说明从 manifest／lock 读取版本；历史文档保留原版本与明确标记。 |
| R30 | release quality job 窄于 check.yml Linux 门禁 | 已修复 | main 与标签共用完整 quality.yml；增加实际安装包／解包启动门禁。 |
| R31 | Arch 与 Intel Mac 已退出自动构建，历史文档仍写五附件/Intel job | 历史政策已标注 | 当前四目标入口与历史 Arch/Intel 记录分开。 |
| R32 | 更新检查链路未提交但已被 just/CI 引用 | 已由基线提交关闭 | 更新服务及 CI 引用在同一基线提交中，干净 checkout 包含完整文件。 |
| R33 | 卸载不刷新 KDE kbuildsycoca6 | 已修复 | 普通用户卸载补充 KDE 缓存刷新，DESTDIR 隔离不变。 |
| R34 | test_arch_package.py 不在 check.yml unittest 列表 | 保留退休范围 | Arch 手工工具测试本地通过；普通 CI 保持四目标支持。 |
| R35 | 硬件探测 recipe 不是自动绿 | 保留实机验收范围 | 物理 DAC、系统媒体键、GPU、门户及跨屏缩放以真实桌面验证。 |
<!-- END_FINDINGS_RESULTS -->

## 6. 后续维护规则

每项新增风险记录验证提交、触发条件、影响、确定性复现、当前状态及验收范围。当前接口分类使用 QML/cxx-qt、进程内消息、本用户 IPC/MPRIS、macOS MediaPlayer 和 GitHub 更新 HTTP；历史报告中的通用 Web 模板术语保留为待整理的报告生成器内容。

优先维持本轮稳定性和数据工作量预算，再安排真实硬件测量、受控文件重命名恢复和分发签名。历史 Arch 与 Intel macOS 工具保留其原记录，自动发布继续遵守四目标范围。

# macOS arm64 文件监听：事件契约与防抖修复

## 失败定位

基线提交：`e4c3c88`。对应质量检查 run：`34132122543`，Linux 与 macOS x86_64 成功，macOS arm64 在 `library::watcher::tests::coalesces_a_burst_of_audio_changes` 失败。

截图中的比较可以简化为：

```text
实际：PathsChanged([<音乐根目录>, <音乐根目录>/track.wav])
断言：PathsChanged([<音乐根目录>/track.wav])
```

`PathsChanged` 表示需要重新核对的文件或目录范围。目录通知是有效输入：它可能包含其他子文件的变化。原生系统可以附带父目录通知、重复通知，并将一次文件操作分到多个送达批次。旧测试要求第一次返回固定的单元素向量，并在随后 750ms 内保持完全安静，因此错误地把某次原生系统的送达形态当成了跨平台协议。

这次截图已经进入测试执行阶段。此前补齐的 Apple 目标导入边界修复继续保留；本轮针对原生事件与测试假设。

## 修复前的可重复证据

在改动前注入受控的目录与文件事件，稳定复现了同样的向量比较失败。此外，同一检查发现并复现了两项独立缺陷：

1. `EventKind::Other + Flag::Rescan` 且路径为空时，会被扩展名/路径过滤器丢弃。
2. 首个原生通知含有超过 2,048 个路径时，会直接进入集合，绕过后续批次才使用的容量检查。

三项修复前检查均失败，退出码 `101`；记录在 `target/qa/watcher-contract-fix/before-fix.log`。目录复现使用等价输入组合：原 CI 日志只保留合并结果，原始 FSEvents flags 与类型未记录。

## 实现规则

### 1. 保留目录失效范围

公开事件仍使用原有 `Changed / PathsChanged / Error`。`PathsChanged` 的注释明确允许文件和目录，单批次路径排序并去重；目录通知继续交给现有 `Library::update_paths` 进行根范围核对。

原始重命名事件保留两端路径，物理根目录映射继续保留用户配置的路径写法。根路径映射保持配置本身的字符串形式。待更新路径的最终排序发生在合并阶段。

### 2. 明确防抖状态

`PendingChanges` 将合并状态与原生监听线程分离，接收显式单调时间戳：

- 最近一次相关事件后等待 500ms。
- 同一批次最多延展到首次相关事件后的 2 秒。
- 无关事件保持现有截止时间。
- 任一批次最多保留 2,048 个唯一路径；首个通知和后续通知使用相同检查。
- 超过容量时清空路径集合并转为全量核对。容量满时的重复路径继续去重。
- `need_rescan()` 在事件类型与文件扩展名过滤之前检查；缺少路径的变更也使用全量核对。
- 全量核对状态覆盖本批次后续的局部路径，批次提交后重置。

事件循环按“停止 → 到期批次 → 原生输入”的优先级选择，持续输入保持截止时间和退出的处理机会。后端通道意外关闭时提交最后一个待处理批次；明确的停止请求优先结束线程。后端错误继续上报给 LibraryService 的既有恢复流程。

容量约束针对合并器保存的路径集合；原生回调使用的输入通道沿用原有实现。

## 测试分层

### 确定性测试

`src/library/watcher/tests.rs` 中的合并测试直接提供事件与时间：精确断言 500ms、防抖延期上限、路径去重、排序、分批、目录加文件、仅目录、重命名两端、重扫标记、容量、重置、通道关闭和退出。

保留 `coalesces_a_burst_of_audio_changes` 名称，由可控事件时间验证一次合并的准确输出。相同输入可以在 Linux、Apple Silicon 和 Intel 上执行。

增加“父目录通知包含尚未单独通知的其他子文件”用例，验证目录范围可以发现两个新增文件，并在目录级通知后删除缺失记录。这个用例约束实现继续保留必要的目录语义。

### 原生文件系统测试

原生监听测试通过实际 `LibraryWatcher` 接收事件，再按业务逻辑调用 `update_paths` 或全量 `scan`，直到数据库满足预期。验证覆盖：

- 创建、修改时长、重命名、删除歌曲。
- 已填充专辑目录移入、重命名、删除。
- 符号链接根目录下的新增与删除，以及索引键保持配置路径。
- 监听器关闭后消费者通道断开。

有限等待用于接收操作系统通知。超时、后端错误、错误索引内容仍然使测试失败；允许父目录、重复与延后送达批次。成功需要数据库内容正确，单独收到任意事件不能算成功。

`tests/library.rs` 中另一个使用同类精确向量断言的集成测试也已修改。它实际消费事件携带的范围。修改时间测试改为设置两个同一秒内的固定时间戳，文件长度保持一致，验证纳秒级失效逻辑，取消对 `sleep(2ms)` 的依赖。

## 回归门禁

`python3 scripts/check-watcher.py --rounds 10` 从 Cargo JSON 构建消息定位本轮编译产生的测试程序，然后重复运行：

- 27 项监听器合并/通道/原生测试。
- 7 项曲库集成测试。

共 34 项/轮。命令要求每轮通过，首次失败立即以非零退出；测试缺失或构建失败也会失败。单独的 Python 测试验证失败会停止、旧的成功报告会被覆盖，以及两个测试程序都确实执行。

Linux、macOS arm64、macOS x86_64 的普通 CI 各增加 5 轮必过检查。macOS 发布前检查也接入同一入口。原生完整 workspace 测试、两种 Apple 目标交叉检查继续保留。

```sh
just watcher-test
# 显式检查单线程调度：
python3 scripts/check-watcher.py --rounds 10 --test-threads 1 --output target/qa/watcher-serial
```

## 验证记录与边界

本轮记录保存在 `target/qa/watcher-contract-fix/`，最终日志放在其 `final/` 子目录。原生 Linux 可执行测试和两个真实 Apple target 的类型/条件编译检查分别记录。

原生 macOS FSEvents 的最终执行结果由推送修复后的 macOS CI 提供；Linux 重放与 Apple 交叉目标检查分别验证事件契约和编译边界。

本轮保持 GUI、图标、封面占位、音频引擎、曲库数据库结构与依赖锁文件原样。运行逻辑改动集中于监听器，其他变动属于测试、CI 与说明。

## 官方依据

- Apple File System Events：目录级通知及事件合并、丢失后的重扫要求：
  <https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/UsingtheFSEventsFramework/UsingtheFSEventsFramework.html>
- notify 8.2.0 `Event`：路径集合、重命名路径顺序与 `need_rescan()`：
  <https://docs.rs/notify/8.2.0/notify/struct.Event.html>

## 本轮已执行结果

- Linux workspace（含 CoreAudio 接口检查）125 项 Rust 测试通过，严格 Clippy 和 Rust 格式检查通过。
- 27 项监听器测试 + 7 项曲库集成测试完成 20 轮四线程、10 轮单线程验证，共 1,020 次测试执行，全部通过。
- 两个真实 Apple 目标的核心库、测试与示例严格 Clippy / 类型检查通过。
- 20 项 Python 回归、23 项 QML 控件用例、release 构建及隔离页面/功能回归通过。
- Wayland 100% / 125% / 150% 和实际托盘协议检查通过。
- actionlint 验证两份工作流通过。

机器可读结果在 `target/qa/watcher-contract-fix/final/summary.json`。新提交的 macOS 原生 CI 运行状态需在推送后确认。

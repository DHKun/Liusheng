# 歌词文件兼容与整句高亮

## 当前显示行为

按本轮体验反馈，播放器统一采用整句高亮。当前句在其行开始时间立即使用正文颜色，其他句使用辅助颜色；暂停、定位、偏移、手动滚动、回到当前句和译文分层继续沿用原有行为。

逐词/逐字渐进渲染、普通 LRC 的近似估算、相关高亮模式设置与 16ms 刷新分支已经撤回。`LyricsView.qml` 使用 Qt Quick 原生 `Text`，颜色直接绑定到当前行，消除了这条路径的颜色缓动。

已有设置中的 `lyric_highlight` 字段按旧配置兼容处理，读取后采用当前整句行为。其余设置和每份歌词的偏移保持原值，后续保存会自然移除已退役字段。

## 保留文件兼容性

为继续读取用户已导入的资料，保留增强 LRC、常见歌词 TTML、明文 YRC/QRC 的受限解析与同名文件查找。当前显示层统一取行开始时间、正文和明确的附文；原始资料内容保留在资料库。

读取优先级继续为用户固定选择、可用本地资料、自动获取资料。损坏的同名文件继续尝试下一来源；加密 QRC 需要先导出为可读取的明文格式。文件大小、行数、XML 节点和时间区间检查保持有效。

导入路径：正在播放页更多菜单 → 查找或更换歌词 → 导入本地。原音乐文件和标签保持原样，原文与译文合并在一个字符串时保留原内容。

## 回归检查

```sh
just lyric-test
python3 -m unittest discover -s tests -p 'test_lyric_delivery.py' -v
python3 scripts/check-ui.py target/release/liusheng --output target/qa/line-lyrics-ui
```

测试覆盖普通 LRC、已有字词格式、偏移、纯文本、长句换行、无缓动整句颜色切换、旧高亮偏好兼容，以及暂停与隐藏窗口后的时钟状态。

历史逐字实验日志保存在 `target/qa/karaoke-completion/`；当前撤回和托盘修复的结果见 `target/qa/line-lyrics-tray/` 与 `docs/LINE_LYRICS_AND_TRAY_FIX.md`。

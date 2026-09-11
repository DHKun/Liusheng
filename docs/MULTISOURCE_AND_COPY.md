# 多来源资料与界面文案精简

本轮基于工作区已有在线资料、限流重试、整句歌词与 Wayland 托盘修复继续迭代。版本保持 0.3.4，标签与远程提交保持原状态。

## 使用

查找窗口和批量补全增加来源选择，默认“全部来源”。手动检索发送歌名、歌手等必要音乐资料；具体来源与数据范围通过“来源与隐私”查看。

| 资料 | 来源 |
| --- | --- |
| 歌词 | LRCLIB、QQ 音乐、网易云音乐 |
| 封面 | MusicBrainz / Cover Art Archive、QQ 音乐、网易云音乐、Deezer |

批量处理保存本次来源选择，取消后的恢复及重启继续沿用它。自动补全原有两个开关继续默认关闭；新增“自动补全也使用 QQ、网易云与 Deezer”独立开关默认关闭，已有安装的自动联网范围保留原有选择。

## 查询与匹配

`providers.cpp` 负责查询参数、来源响应与候选身份；`multi_lookup.cpp` 负责来源轮询、结果合并和按需下载。

- 含中日韩字符的检索优先 QQ / 网易云；其他检索优先原有来源。封面额外查询 Deezer。
- 每个来源独立解析错误码与数据格式。多来源模式下，冷却中的来源直接跳过；失败保留该来源诊断，并继续查询其他来源。有效候选始终可以保留。
- 单来源模式延续有界自动重试及 Retry-After 等待。多来源每次请求最多 7 秒；原有来源的扩展查询按 15 秒预算在查询之间检查，所有请求有取消代次与结束边界。
- 每个来源最多展示 20 个候选，总列表最多 40 个。按来源 ID 去重，同一歌曲的不同版本与来源仍可分别预览。
- 只有所有选中来源成功完成，结果才作为完整搜索缓存保存。部分来源失败时的空结果保持为错误状态，绝不写成“歌曲不存在”的缓存。
- 自动应用要求歌名、歌手、版本、时长及已知专辑匹配，且候选唯一。来源覆盖不完整时保留人工确认。Live、演出年份、短版与混音版本继续参与判断，音频标签保持原样。

QQ / 网易云先取得歌曲资料，用户预览时再获取歌词正文。候选尚未完成歌词读取时，应用按钮保持禁用；取得正文后再判定同步、纯文本或来源明确标注的纯音乐。QQ 歌词使用严格 Base64 与 UTF-8 检查；已有附文独立保留时间轴，界面继续采用整句高亮。

QQ 封面使用专辑标识生成图片地址。网易云搜索缺少图片时查询歌曲详情，并核对歌曲与专辑标识后取得图片。Deezer 可能以英文名称提供中文作品，别名存疑的候选保留手动确认。

## 网络与资源边界

生产环境使用固定的 HTTPS 主机与元数据路径白名单，保留 TLS 验证，限制重定向、响应字节、图片尺寸与歌词行数。查询接口使用应用自己的 User-Agent 和正常 Referer。应用无需用户账号或 Cookie，音频播放地址接口保持在范围外。

图片协商使用 JPEG / PNG。真实接口验收发现 QQ CDN 会在 `.jpg` 地址根据 Accept 返回 WebP；最小 Qt 安装可能没有 WebP 插件。请求只声明所有发布包具备的格式，避免“响应成功而预览失败”。本地 WebP 导入继续由宿主实际 Qt 图片插件决定。

新增图片地址限定于网易云图片 CDN、QQ 专辑图片路径和 Deezer cover 路径。部分来源返回明文图片地址时，仅将已知 CDN 地址规范化为 HTTPS，然后再次执行完整地址校验。

所有结果继续使用原有私有目录、原子写入、内容摘要和绑定机制。原音频、标签、音乐目录、路径身份与歌词偏移保持原样。图片只有通过解码后才能预览或保存。

QQ / 网易云适配参考成熟播放器的公开元数据接口使用方式，服务方可能调整接口、地区策略或访问限制；本实现没有长期稳定性承诺。来源的版权与使用条件保持有效，后续商业发行应单独审核使用授权。任何来源的认证失败、拒绝访问或失效都以独立错误处理。

## 界面

查找窗口保留检索字段、来源选择、候选、预览和应用动作；长篇联网说明、详细查询词和来源错误归入“来源与隐私”。结果行保留歌曲、歌手、专辑和来源，演唱版本差异只在预览区提示一次。

批量窗口使用简短进度、状态与操作按钮。逐行长错误通过悬停查看，整体诊断可在资料说明中查看；成功状态避免重复解释存储策略。等待倒计时、取消、重试、失败状态继续可见。

空歌词页改为“暂无歌词”与查找入口；空曲库改为“暂无音乐”；侧边栏仅在扫描或出错时显示状态。设置标签与说明压缩为短句，保留路径格式、设备限制、隐私和保存结果。

全部新增对话框、来源下拉框沿用窗口内 Popup.Item。Esc 先关闭来源详情，再返回原查找窗口；用户输入和预览状态继续保留。

## 验证

常规 CI 使用私有本地 HTTP 夹具验证所有来源响应、错误隔离、取消、缓存、预览、图片协商与持久化。原生 Qt 测试在 Linux 和 macOS arm64 的共享工作流运行。源码中的 `LIUSHENG_ONLINE_TEST` 地址注入仅存在于独立测试程序。

```sh
python3 scripts/check-online-assets.py --output target/qa/multisource/native
python3 scripts/check-online-assets.py --compiler clang --output target/qa/multisource/clang
python3 scripts/check-controls.py
# 明确联网：生产请求路径、固定公开样本、临时目录，输出仅含元数据与验收状态。
QT_QPA_PLATFORM=offscreen target/qa/multisource/native/build-default/online_assets_test --multi-live
```

本轮验收记录位于 `target/qa/multisource-refinement/`。`before.json` 保存改动前文件摘要，`live.log` 保留首次发现 WebP 协商问题的结果，`live-final.log` 为修正后真实接口检查，`final/` 为完整回归与安装包验收。最终通过情况以 `summary.json` 为准。

来源 API 的长期覆盖率、版本匹配准确率和不同地区网络性能需要持续样本测试；原生 macOS 系统控制中心、KDE 面板与物理设备仍由原生 CI / 实机验收。

## 接口与实现参考

- OpenLyrics QQ 适配器：https://github.com/jacquesh/foo_openlyrics/blob/main/src/sources/qqmusic.cpp
- OpenLyrics 网易云适配器：https://github.com/jacquesh/foo_openlyrics/blob/main/src/sources/netease.cpp
- Strawberry Deezer 封面适配器：https://github.com/strawberrymusicplayer/strawberry/blob/master/src/covermanager/deezercoverprovider.cpp
- LRCLIB：https://lrclib.net/docs
- MusicBrainz：https://musicbrainz.org/doc/MusicBrainz_API
- Cover Art Archive：https://musicbrainz.org/doc/Cover_Art_Archive/API

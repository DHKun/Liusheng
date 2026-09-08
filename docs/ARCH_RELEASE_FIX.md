# 0.3.1 · Arch 发布依赖与安装验证

## 故障定位

`v0.3.0` 指向 `2d5ddb2`，普通 CI 已通过。Release packages 的运行 `34172378542` 中，workspace、DEB、RPM 和两个 macOS 打包任务通过；Arch 在 makepkg 依赖检查阶段退出，缺少 `qt6-svg` 与 `qt6-wayland`，退出码 8。

`packaging/arch/PKGBUILD.in` 已声明两个运行依赖，旧 Release 工作流仍使用单独维护的旧 pacman 安装清单。makepkg 同时检查运行与构建依赖，故在 Rust 编译之前停止。Publish job 的 `needs: [deb, rpm, arch, macos]` 维持完整发布门禁，Arch 失败时被跳过。

## 修复

1. **单一依赖来源。** 普通构建用户通过 `makepkg --printsrcinfo` 读取渲染后的 PKGBUILD，`scripts/arch-dependencies.py` 解析运行、构建、测试及 x86_64 专属依赖。CI 根据 `packages.txt` 安装，随后用 `pacman -T` 核对保留版本约束的 `dependencies.txt`。依赖清单生成失败会立即阻止后续步骤。
2. **共享完整打包流程。** `.github/workflows/package-arch.yml` 是普通 CI 与 Release 的共同入口。独立 Arch 容器完整更新系统；安装依赖由容器 root 执行，源码与 makepkg 元数据由普通 builder 用户处理。编译继续启用 makepkg 的依赖和源码校验。
3. **安装后验收。** 新流程实际 `pacman -U` 安装产物，核对版本、架构、运行依赖、程序、desktop 启动器及所有图标文件，再以普通用户启动 `/usr/bin/liusheng` 的隔离初始场景与 QML 控件测试。通过后才上传 `liusheng-arch`。
4. **确定的构建目录。** PKGBUILD 的构建与安装阶段都使用同一个源码隔离目录中的 Cargo target。包输出、源码缓存与 makepkg 工作目录明确绑定本次临时目录。复制导致安装脚本执行位丢失时，显式 Bash 入口仍可执行。DEB/RPM 包装脚本也向 Cargo 和安装器传递同一 target 目录。
5. **完整附件验收。** Publish 保留所有平台依赖，`scripts/check-release-assets.py` 在发布前要求同一版本的 DEB、RPM、Arch、macOS arm64、macOS x86_64 五份非空附件，拒绝重复、缺失与混入旧版本，生成并复核 SHA256SUMS。发布 action 同时启用 `fail_on_unmatched_files`。

新增可移植测试位于 `tests/test_arch_package.py`，覆盖依赖解析与故障传播、受控构建/安装命令、缺失执行位、目标目录隔离、两个 workflow 共用入口、版本与锁文件一致性，以及 Release 附件缺失/重复/旧版本等情况。

## 版本与重新发布

两个 crate 与 Cargo.lock 中本地包版本同步为 **0.3.1**，第三方依赖版本保持原样。已有 `v0.3.0` 标签保留。GitHub 对旧运行的 Re-run 继续使用原来的 SHA 与 ref，因此修复通过新提交和新标签发布。

```bash
# 当前修复提交先推送 main，等待普通 CI（含新增 Arch 打包）通过。
git push origin main

# 确认该提交的 CI 通过后再执行：
git tag -a v0.3.1 -m "留声 v0.3.1：修复 Arch 发布依赖与安装验收"
git push origin refs/tags/v0.3.1
```

新标签触发 Release packages；全部打包任务与 publish 成功后，检查五个平台附件及 SHA256SUMS。

## 验证记录

本轮日志位于 `target/qa/arch-release-fix/`。Arch 依赖与真实构建采用独立官方 Arch bootstrap 根文件系统，现有 Fedora 安装和用户音乐数据保持原样。可移植命令替身测试与真实 makepkg/安装验证分别记录，最终结果以该目录中的验证日志为准。

## 上游依据

- makepkg 的运行/构建依赖检查：<https://man.archlinux.org/man/makepkg.8>
- pacman 的依赖约束核对：<https://man.archlinux.org/man/pacman.8>
- GitHub 重跑继续使用原 SHA/ref：<https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs>

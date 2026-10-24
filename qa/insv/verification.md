# INSV 集成验证

日期：2026-10-24。

## 已验证

- `cargo test --manifest-path src-tauri/Cargo.toml insv_ -- --nocapture`：3 通过、0 失败，未运行无关测试。
- 双文件 `_00_` / `_10_` 导入去重，包括扩展名大小写不同的情况；旧预设缺少全景字段时使用兼容默认值，新字段可序列化保存。
- 真实 FFmpeg 构造的双视频轨 MP4（使用 `.insv` 扩展名）可识别为待拼接全景，默认输出 MP4，ProRes 默认 MOV。
- 用 FFmpeg 横向合并两个构造视频轨模拟拼接边界，最终视频可以真实编码，写入全景标记并由 ffprobe 回读验证；SDK 输出校验可拒绝明显截断和位深降级。
- 模拟 SDK 返回 `error:` 但退出码为 0 时仍判定失败；进程开始输出后进入静默等待时，取消能及时终止进程。测试独立于 PowerShell 首次启动耗时。
- TypeScript / Vite 生产构建通过；Tauri Windows release 便携构建通过。
- 浏览器演示界面中，FlowState + 方向锁定、动态拼接及关闭色彩融合可以存入新预设，重新加载并应用后仍保持。
- 1180×760 下四个页签均可见，文档宽度 1180，无页面水平溢出；控制台没有 error/warn。
- 界面预览：`panorama.jpg`，使用浏览器内置演示媒体，不含真实用户媒体。
- 便携构建保留程序旁配置及已部署 SDK；前端输出移到 `dist/web`，避免 Vite 清空便携目录。

## 当前依赖阻塞与未验证范围

- 本机仅找到 Insta360 Studio，未找到独立的官方 Desktop Media SDK。
- 官方 GitHub 仓库不包含 SDK 二进制下载资产。官方 SDK 获取说明要求先申请并经过审核，通过后邮件提供下载链接。
- 实际打开官方 SDK 下载管理页会跳转到账号登录，当前浏览器没有已登录账号；未提交申请、未接受服务协议。
- SDK 的具体 CLI、DLL、模型、机型支持和性能仍待获得官方包后核验。缺少 SDK 时 INSV 拼接导出会明确失败，不会退化为鱼眼转码。
- 未验证真实 INSV 样片、镜头标定、FlowState、防抖效果、方向锁定、AI／动态拼接画质、降噪、HDR 拍摄模式、真实音轨及原生桌面操作。构造文件不代表这些能力已经验收。
- 两阶段流程使用有损 HEVC 中间文件，会增加一次编码和临时磁盘占用；当前预检查计入中间文件空间。

官方参考：

- https://onlinemanual.insta360.com/developer/en-us/resource/sdk
- https://insta360develop.github.io/Insta360-Developer_Docs/en/x/desktop/media/

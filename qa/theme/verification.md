# 界面配色

2026-10-24

- 默认主题为爱蜜莉雅紫。内置爱蜜莉雅紫、深海青蓝、樱花暮色、林间薄荷、落日琥珀、银白紫罗兰六套预设。
- 设置中可分别组合背景、面板、主要文字、次要文字、强调色和操作按钮色。按钮及选中框前景色随填充自动选择。
- 修改实时预览；关闭设置恢复已保存配色，保存后写入现有 preferences 配置。旧配置缺少主题字段时自动使用默认紫色。
- 前端 TypeScript/Vite 与 Tauri Windows release 构建通过。
- 使用 Edge headless 验证默认主题、浅色预设、按钮文字、取消还原、自定义保存并重新加载、恢复默认、旧配置兼容与 1180×760 布局，浏览器无 pageerror。
- 已检查 emilia-settings.png、light-settings.png 的实际截图。
- 已更新 dist/VideoSizeComposer/VideoSizeComposer.exe，文件大小和修改时间与构建产物一致；保留便携目录原有 config、工具与资源。

验证边界：未启动原生桌面窗口；浏览器验证使用演示数据，不代表真实视频编码或原生 WebView2 交互验收。

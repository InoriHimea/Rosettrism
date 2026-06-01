# Requirement History

## v1.0.0 — 2026-06-01 — QQ/QRC Karaoke 实际播放修正

### 目标
- 使用真实 QQ 音乐《龙战骑士》数据验证歌词播放，而非仅依赖手写样例。
- 默认歌词渲染模式调整为 Karaoke。
- 顶层 unified JSON 必须保留 QQ QRC 的逐字 timing，不能被无逐字 timing 的 inline lines 降级。
- Karaoke 舞台必须随当前 active line 连续滚动，并保持左右交替布局。
- 当前逐字填充进度、重音/换气/长音等助唱标记必须锚定到对应字。

### 验收条件
- 真实《龙战骑士》JSON 识别为 Tencent / QRC。
- `迎着风极速在超越` 在 51680ms 显示为 active line；`迎` 已完成、`着` 部分推进、`风` 尚未推进。
- Karaoke 容器滚动到 active line 附近，而非保持顶部不动。
- `迎` 的重音点和标签在对应字附近正确显示。
- 前端构建成功；实际 App dialog 可从搜索结果进入并显示播放舞台。

### 实现状态
- [x] 真实 QQ/QRC 数据拉取与复现
- [x] unified normalization 优先保留 QRC word timing
- [x] 默认 Karaoke 模式
- [x] Karaoke 连续滚动队列与 active line 跟随
- [x] 逐字填充和助唱标记真实数据 smoke
- [x] 实际 App dialog 浏览器验收
- [x] 发布检查与提交

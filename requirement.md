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

## v1.1.0 — 2026-06-05 — AI 优选可追踪化

### 目标
- 将 AI 歌词优选从前端与数据库中的“预留入口”升级为可审计、可回放的评分记录。
- 在聚合候选评分后保存模型、base URL、候选摘要 hash、`best_index`、各候选 heuristic/AI 分数、原因和创建时间。
- 通过 `/api/stats` 与 `/api/cache/:id` 暴露最近评分和统一缓存对应评分明细，让仪表盘 Quality/Inspector 视图可直接显示。

### 验收条件
- `aggregate_fetch` 在成功写入 `unified_cache` 后写入对应 `ai_scores`。
- `src/cache.rs` 提供明确的 `put_ai_score(unified_cache_id, score_json)` 与 `list_ai_scores(unified_cache_id)` 方法。
- 前端 Inspector 显示候选来源、heuristic score、AI score、reason 与最终选中来源，不再显示“预留”文案。
- README / README-zh 同步说明 AI 优选是可追踪功能。

### 实现状态
- [x] AI 评分结果结构与候选摘要 hash
- [x] `ai_scores` 写入和查询方法
- [x] `/api/stats`、`/api/cache/:id` 评分明细输出
- [x] Inspector AI 评分表格
- [x] 文档同步

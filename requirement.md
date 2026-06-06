# Requirement History

## 需求追加格式

每次追加需求时，建议在本文件末尾新增一个版本化章节，格式如下：

```markdown
## vX.Y.Z — YYYY-MM-DD — 主题

### 目标
- 本次追加需求要达成的结果。

### 验收条件
- 可验证的完成标准。

### 实作状态
- [ ] 待处理项目。
- [ ] 已完成项目。

### 关联 plan
- `.plan/YYYY-MM-DD-topic.md`
```

建议规则：

- `vX.Y.Z` 遵循 semantic versioning；功能新增通常提升 minor，修补提升 patch，破坏性变更提升 major。
- `YYYY-MM-DD` 使用需求确认或开始规划的日期。
- `主题` 使用简短、可读的人类描述。
- `目标` 与 `验收条件` 应尽量可验证，避免只描述实现细节。
- `实作状态` 随开发进度更新，完成后应与对应 `.plan/` 文件互相呼应。
- `关联 plan` 必须指向本次开发前建立的时间命名计划文件。

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

## v1.2.0 — 2026-06-05 — Dashboard Server Token 驗證整合

### 目標
- 讓 Dashboard 在 `ROSETTRISM_SERVER_TOKEN` 啟用時仍可安全呼叫 API。
- 將前端 API 呼叫集中到共用 client，統一注入 `x-rosettrism-token` 與 JSON 錯誤解析。
- 在設定頁提供 Server Token 輸入與清除流程，並避免長期保存敏感 token。

### 驗收條件
- 遠端綁定且啟用 token 時，Dashboard 可透過設定頁 token 成功呼叫 `/api/*`。
- 未帶 token 或 token 錯誤時，後端回傳可讀的 401 JSON 錯誤。
- README / README-zh 說明 Dashboard token 使用方式與 `sessionStorage` 儲存策略。

### 實作狀態
- [x] `frontend/src/api/client.js` 共用 API client
- [x] `sessionStorage` token 讀寫與清除
- [x] Settings 頁 Server Token 欄位
- [x] API 401 錯誤訊息改善
- [x] 文件同步

### 關聯 plan
- `.plan/2026-06-06-six-version-completion-audit.md`（歷史需求補登與合併後複查）

## v1.3.0 — 2026-06-05 — fetch_runs 任務紀錄與可觀測性

### 目標
- 將 `fetch_runs` 從資料表預留升級為實際任務紀錄系統。
- 記錄聚合 fetch、多來源 search、指定結果 fetch 與聚合成員 fetch 的 query、source、mode、status、message 與 created_at。
- 透過 API 與 Dashboard 展示近期任務與狀態分布。

### 驗收條件
- `src/cache.rs` 提供 start / finish / list / status count 方法。
- `src/service.rs` 在主要 fetch/search 路徑建立並完成 fetch run。
- `/api/runs` 與 `/api/stats` 可輸出近期任務與狀態分布。
- Overview / Cache 視圖可展示最近任務。

### 實作狀態
- [x] `fetch_runs` 寫入與查詢方法
- [x] service fetch/search 路徑接入
- [x] `/api/runs` 與 stats 輸出
- [x] Dashboard 最近任務面板
- [x] README / README-zh 可觀測性說明

### 關聯 plan
- `.plan/2026-06-06-six-version-completion-audit.md`（歷史需求補登與合併後複查）

## v1.4.0 — 2026-06-05 — Dashboard 前端拆分與設計系統基礎

### 目標
- 將過大的 `App.jsx` 與單一 CSS 拆分為 views、hooks、api client、i18n 與分層 styles。
- 降低後續科技感 UI、動畫與 plugin/skill 化改造的耦合風險。
- 保留現有播放與搜尋行為，優先完成結構性重構。

### 驗收條件
- `frontend/src/App.jsx` 不再承載大部分 view 實作細節。
- Overview、Fetch、Cache、Inspector、Settings 皆有獨立 view 檔案。
- API 呼叫、AI 設定、lyric 設定、cache entries、sidebar 狀態皆可由獨立模組追蹤。
- 樣式已拆分為 tokens、layout、components、lyric-stage。

### 實作狀態
- [x] views 拆分
- [x] hooks 拆分
- [x] API client 拆分
- [x] i18n 字典拆分
- [x] styles 分層
- [x] 前端 build 驗證

### 關聯 plan
- `.plan/2026-06-06-six-version-completion-audit.md`（歷史需求補登與合併後複查）

## v1.5.0 — 2026-06-05 — requirement 與 .plan 工作流制度化

### 目標
- 建立 `.plan/` 目錄規範，讓每次開發前的需求、階段、驗收與測試紀錄可版本追蹤。
- 建立 plan template，降低後續維護者漏寫需求與任務狀態的風險。
- 在 README-zh 補充開發工作流，優先採 zh-TW 描述。

### 驗收條件
- `.plan/README.md` 說明命名規則與使用流程。
- `.plan/TEMPLATE.md` 包含背景、目標、非目標、風險、階段 checklist、驗收條件、測試紀錄與完成狀態。
- `requirement.md` 定義版本化追加格式。
- README-zh 有開發工作流章節。

### 實作狀態
- [x] `.plan/README.md`
- [x] `.plan/TEMPLATE.md`
- [x] `requirement.md` 追加格式
- [x] README-zh 開發工作流
- [x] `.plan` 納入版本追蹤

### 關聯 plan
- `.plan/2026-06-06-six-version-completion-audit.md`（歷史需求補登與合併後複查）

## v1.6.0 — 2026-06-05 — Unified JSON Schema 與客戶端相容性策略

### 目標
- 為 `UnifiedLyric` 建立正式 JSON Schema 與相容性說明。
- 在聚合輸出中提供 `schema_version`，讓客戶端可依版本做降級策略。
- 以 fixtures 與測試驗證典型來源輸出仍能符合 schema 與 Rust model。

### 驗收條件
- `schema/unified-lyric.schema.json` 描述核心 UnifiedLyric 結構。
- `docs/unified-json.md` 說明 tracks、inline、annotations、ruby、translation、reading、romanized 的解析規則。
- `tests/unified_schema.rs` 驗證 schema 本身與 fixtures。
- README / README-zh 指向 schema 與客戶端建議解析策略。

### 實作狀態
- [x] `schema_version` 欄位
- [x] JSON Schema 文件
- [x] Unified JSON 相容性文件
- [x] 多來源 fixtures
- [x] schema / model 測試
- [x] README / README-zh 同步

### 關聯 plan
- `.plan/2026-06-06-six-version-completion-audit.md`（歷史需求補登與合併後複查）

## v1.7.0 — 2026-06-06 — 六大版本完成度複查與後續優化評估

### 目標
- 重新檢查 v1.1.0 至 v1.6.0 六項大版本成果是否已有可追蹤落點。
- 補齊已合併功能在 `requirement.md` 中缺少的版本化歷史。
- 產出後續可優化空間，協助下一輪開發排序。

### 驗收條件
- 新增完成度複查報告，列出六大版本狀態、主要落點與複查判斷。
- 本次 plan 已建立並標記各階段完成。
- 後續優化建議按短期、中期、長期分類。
- Rust 測試與前端 build 通過。

### 實作狀態
- [x] 六大版本完成度矩陣
- [x] v1.2.0 至 v1.6.0 需求歷史補登
- [x] v1.7.0 複查需求追加
- [x] 後續優化建議整理
- [x] CLI fixture 路徑跨平台修正
- [x] 測試與 build 驗證

### 關聯 plan
- `.plan/2026-06-06-six-version-completion-audit.md`

## v1.8.0 — 2026-06-06 — README 开源格式与 AGPLv3 授权同步

### 目标
- 将 README / README-zh 调整为常见开源项目格式，包含项目简介、功能特性、安装、快速开始、CLI/API、路线图、开发、贡献与协议。
- 明确回答当前项目在一轮计划完成后仍建议继续做功能迭代与优化，并按短期、中期、长期给出优先方向。
- 将项目授权 metadata 与仓库 LICENSE 统一为 AGPLv3。

### 验收条件
- README / README-zh 可作为开源项目首页使用，并保留原有核心使用说明。
- README / README-zh 明确指向完成度复查报告与后续优化路线图。
- `Cargo.toml` 与 `frontend/package.json` 的 license 字段为 `AGPL-3.0-only`，与 AGPLv3 LICENSE 一致。

### 实作状态
- [x] README 开源格式重写
- [x] README-zh 开源格式重写
- [x] 后续迭代与优化路线图补充
- [x] Rust crate license metadata 更新
- [x] Frontend package license metadata 更新

### 关联 plan
- `.plan/2026-06-06-readme-agpl-open-source-refresh.md`

## v1.9.0 — 2026-06-06 — Plan / Requirement 一致性检查

### 目标
- 在进入下一阶段开发前，为当前已修改优化版本建立 release tag，并继续推进短期优化计划。
- 新增轻量工作流检查，提醒维护者在修改功能相关文件时同步更新 `requirement.md` 或 `.plan/`。
- 支援本地 staged 检查与 CI / 分支检查常用的 base ref 对比模式。

### 验收条件
- 当前 HEAD 已建立 `v4.8.13` annotated tag，作为上一轮优化版本切点。
- `scripts/check-plan-requirement.sh` 可检查 feature-sensitive 变更是否伴随 `requirement.md` 或 `.plan/` 更新。
- README / README-zh 的推荐检查命令包含该脚本。
- 本次 plan 文件记录 tag、实现与测试结果。

### 实作状态
- [x] 建立 `v4.8.13` annotated tag
- [x] 新增 plan / requirement 一致性检查脚本
- [x] README / README-zh 开发检查同步
- [x] 本次 plan 与需求历史追加
- [x] 脚本、格式与 Rust 测试验证

### 关联 plan
- `.plan/2026-06-06-plan-requirement-consistency-check.md`

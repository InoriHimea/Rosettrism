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

### 測試基線追加目標（2026-06-06）
- 為 Server API 建立 Axum router 層級測試，覆蓋 `/api/health`、`/api/stats`、`/api/runs`、`/api/cache/:id`。
- 固定 Server Token 驗證：未帶 token 回 `401` JSON；`x-rosettrism-token` 與 `Authorization: Bearer` 正確時通過。
- 為 `cache_detail` handler 固定 upstream、unified、missing id 三種情境。
- Rust 測試基線明確使用 localhost proxy bypass：`NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost cargo test`。
- Frontend 至少提供 `npm test` smoke script，驗證 Dashboard 可載入、Settings token input 可操作、Fetch 頁可渲染。

### 測試基線驗收結果（2026-06-06）
- [x] `src/server.rs` 已新增 Axum router integration tests，涵蓋指定 API endpoint。
- [x] Server Token 401 JSON、`x-rosettrism-token`、`Authorization: Bearer` 三種情境已納入測試。
- [x] `cache_detail` upstream / unified / missing id 三種分支已納入測試。
- [x] `frontend/package.json` 已新增 `test` script，並以 Playwright smoke 覆蓋 Dashboard / Settings / Fetch 基線。
- [x] Rust 測試與 frontend build 通過；`npm test` 在目前容器因缺少 Playwright Chromium 且 CDN 下載 403 而記錄為環境限制。
- [x] 本次補強 plan 已建立：`.plan/2026-06-06-v1.8-test-baseline.md`。

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

## v2.0.0 — 2026-06-12 — Dashboard 与歌词播放 UI/UX 现代化改造

### 目标
- 将前端体验从“可用且有风格的控制台”提升为“成熟、现代、带轻科技感的歌词工具产品界面”。
- 保留现有 R 角、歌词舞台和工具型信息密度，继续深化日系轻科技配色，避免回到蓝紫主导。
- 每个阶段必须可独立验证、可回滚、可发布，并留下截图或 Playwright 视觉回归证据。

### 验收条件
- 总览页首屏形成清晰诊断流，桌面 1280x720 可同时看到状态卡、缓存健康和多个辅助模块。
- 移动端 390x844 首屏能看到导航、关键状态和一个主要内容模块，不出现横向滚动、遮挡或文本溢出。
- CSS 设计 token 覆盖颜色、R 角、阴影、边框、间距、字号、动效，常用控件状态一致。
- 页面观感稳定落在米白、墨绿、樱色、金茶、青绿的日系轻科技方向。
- 歌词播放页保持 QQ 音乐参考行为，助唱标注锚定正确，换气、重音、长音高度统一，气泡独立消散。
- 获取页与设置页从长表单堆叠升级为更清晰的检索台和控制分组。

### 实作状态
- [x] REQ-UI-001：仪表盘现代化与信息密度优化
- [ ] REQ-UI-002：设计 Token、R 角与组件一致性
- [ ] REQ-UI-003：日系轻科技配色系统深化
- [ ] REQ-UI-004：歌词播放页产品化精修
- [ ] REQ-UI-005：移动端响应式与首屏效率
- [ ] REQ-UI-006：获取页与设置页产品化

### 关联 plan
- `.plan/20260612-152430-ui-ux-modernization.md`


## v2.1.0 - 2026-06-13 - 质量评分提升到 9 分

### 目标
- 将当前约 8.4 的综合质量状态提升到 9.0+，优先补齐真实多语言歌词回归覆盖、前端歌词播放可维护性、文档/计划/发布现场整洁度。
- 覆盖国语、粤语、日语、英语四类歌词播放场景，并用可复现 fixture 固定关键行为。
- 保留可选 live capture 路径，用于发布前从真实 Provider 拉取《龙战骑士 - 周杰伦》《海阔天空 - Beyond》《ブルーバード - いきものがかり》《Just One Last Dance - Sarah Connor》的最新数据证据。

### 验收条件
- 新增或更新自动化测试，常规测试使用固定 fixture，不依赖外部网络、版权接口或 Provider 限流。
- Playwright 覆盖四语种播放视图：能打开播放视图、首屏无遮挡、逐字进度存在、注解不重叠、移动端不横向溢出。
- `LyricPlaybackView.jsx` 中倒计时、双行 lane、meta line、annotation label、进度计算等复杂纯逻辑拆出为可单测模块。
- `frontend/test-results/` 不纳入提交；live capture 输出写入被忽略的验证目录；`frontend/playwright-artifacts/*.png` 仅在有意更新视觉基线时提交。
- 必跑验证通过：`cargo fmt --check`、`cargo test --no-fail-fast`、`npm run build`、`npm run test:unit`、`npm test`、新增多语言回归命令、`git diff --check`。
- 本轮新增 `requirement.md` 与 `.plan/2026-06-13-quality-to-9.md` 内容保持 UTF-8 中文可读。

### 实作状态
- [x] 质量提升需求与计划文档建立
- [x] 歌词播放纯逻辑拆分与单测补充
- [x] 四语种 fixture 与 Playwright 回归补充
- [x] live capture 脚本与忽略目录策略补充
- [x] 必跑验证完成并记录结果

### 关联 plan
- `.plan/2026-06-13-quality-to-9.md`

## v2.2.0 - 2026-06-21 - 4.8.20 前端播放与来源选择收口

### 目标
- 将 `v4.8.19` 之后的歌词播放渲染、来源选择器、Playwright 断言和前端构建产物整理为可发布的 `v4.8.20`。
- 修正旧 UI/UX 计划中与已发布 `v4.8.19` 冲突的版本映射。
- 清理 README 路线图中已经完成的 Provider 健康度、缓存维护、结构化 API 错误等旧待办表述。

### 验收条件
- Karaoke 播放页标题、元信息、倒计时气泡、逐字节点和移动端不溢出行为有自动化断言覆盖。
- Fetch 页来源选择器在 Provider 数量较多时不遮挡操作区，键盘/点击关闭行为可用。
- `Cargo.toml`、`Cargo.lock`、`frontend/package.json`、`frontend/package-lock.json` 同步为 `4.8.20`。
- `frontend/dist` 已由当前源码重建。
- 必跑验证通过，或记录明确环境阻断：`cargo fmt --check`、`cargo test --no-fail-fast`、`npm run build`、`npm run test:unit`、`npm test`、`npm run verify:meta-stress`、`git diff --check`。
- `frontend/verification/` 继续作为本地证据目录，不纳入本次提交。

### 实作状态
- [x] 收口计划建立
- [x] 版本号与路线图同步
- [x] 前端构建产物重建
- [x] 自动化验证完成并记录 Windows 策略阻断
- [x] 提交、打 tag、推送远端

### 关联 plan
- `.plan/2026-06-21-frontend-playback-source-closure.md`

## v2.3.0 — 2026-07-28 — QQ 音乐式歌词渲染方向性改进

### 目标
- 在不复制 QQ 音乐品牌素材、专有资源或私有接口的前提下，对标其成熟歌词播放的通用交互规律。
- 将当前播放器从“功能齐全的独立预览”升级为可由真实媒体时钟驱动、状态确定、双行稳定接力、逐字贴音的歌词舞台。
- 收敛背景、粒子、渐变、阴影、活动竖线和标注图例的视觉竞争，确保活动歌词始终是第一视觉焦点。
- 固化中文、粤语、日语、英语长句，以及翻译、读音、ruby、助唱标注、长空拍和响应式场景的行为契约。
- 建立专用 playback harness、固定时间点行为断言、人工确认截图 baseline 和性能采样组成的验收闭环。

### 验收条件
- 播放层提供统一 clock adapter，preview clock 与 audio/media clock 可替换；seek、暂停恢复、后台恢复和重播均从 clock 真值重新计算状态。
- deterministic playback frame state 统一输出 phase、active/next line、lane、逐字/整行进度、倒计时和可见标注；连续播放 10 分钟无累计时钟漂移。
- 逐字边界偏差不超过一帧预算或 50ms（取较大值）；seek 后 100ms 内活动行、进度、倒计时和标注一致。
- 双行 lane 物理位置稳定，下一句提前就位，切行无左右跳位、闪烁或布局塌缩；旧行退场时间控制在 180–320ms。
- 中文、粤语、日语、英语长句在 390x844、768x1024、1280x720、1440x900 下无横向溢出；移动端主要触控目标不小于 44×44px。
- 助唱标注贴字但不压字；同锚点和高密度场景按优先级降级；翻译、读音、ruby 和罗马音不导致活动 lane 跳动。
- 默认舞台为低噪声的 Rosettrism 自有视觉，不以大面积蓝紫或 QQ 音乐皮肤复制作为对标结果；`prefers-reduced-motion` 下歌词状态语义保持完整。
- 新增歌词专项自动化，不依赖 URL hash 假设导航；截图按 fixture、viewport、固定时间点冻结，并在更新 baseline 前人工审阅。
- 60 秒自动播放采样无持续掉帧趋势，测试环境内无由歌词渲染造成的 >100ms 长任务；连续进度更新不重渲染无关 Header、Legend 和 Controls。

### 实作状态
- [x] 当前实现复核与 QQ 音乐式体验差距矩阵
- [x] P0 播放内核 → P1 舞台行为 → P2 视觉产品化路线确定
- [x] 阶段任务、量化验收、场景矩阵、风险和改动落点规划
- [ ] Phase 0：参考证据与 before baseline 冻结
- [x] Phase 1：clock adapter、状态机与高频渲染解耦
- [x] Phase 2：稳定双行 lane、逐字边界与长句适配
- [x] Phase 3：前奏、空拍、助唱标注与多语副行（运行时代码、18/18 单测、隔离构建与 Firefox 浏览器专项分段验收通过，覆盖 12 个场景）
- [x] Phase 4：低噪声舞台与播放器控件产品化（默认低噪声、文字层级 token、控件主次重排；18/18 单测、Firefox 默认态/移动端/reduced-motion 专项与隔离构建通过）
- [x] Phase 5：专用自动化、截图 baseline 与性能验收（固定时钟 harness、四 viewport baseline、200 行与 60 时间点性能采样；21/21 单测、Firefox benchmark 7/7、隔离构建通过，Three.js 按需分包）
- [ ] Phase 6：真实 QQ/QRC 数据 A/B、完整构建与发布收口（真实/逐行/raw 回归、Windows Edge 4/4、Firefox 4/4、Chrome 四项断言、Rust 165/165、正式 dist 重建均通过；待产品负责人确认截图/录屏后签字关闭）
- [ ] Phase 7：真实媒体播放闭环（media clock、HTMLAudioElement 包装层、本地 WAV harness、Edge/Chrome 各 4/4 通过；Firefox 业务场景完成但 runner 回收异常；正式 Provider 暂无合法音频 URL）
- [x] Phase 8：歌词质量与数据可信度（`word_timed`/`line_timed`/`unsynced`/`invalid` 分级、能力矩阵、结构诊断、可解释降级和 UI 状态已落地；25/25 单测、Chrome 发布专项 5/5、多语与播放核心 13/13、正式 dist、依赖审计 0 漏洞通过）
- [x] Phase 9：播放器产品会话能力（会话内核 35/35 单测、Chrome 专项 5/5、歌词核心 13 项全执行完成但 runner 回收返回 1、Rust 165/165、正式 dist、依赖 0 漏洞、格式与文档一致性通过）

### Phase 9 验收追加

- 播放队列和顺序/单曲循环/列表循环/随机模式必须由独立纯函数内核管理，不侵入媒体 clock 和歌词 frame state。
- 状态恢复只接受当前队列中的 durable 音源；临时签名 URL、过期状态和缺失队列项不得被持久化复活。
- 音量、静音、倍速、当前歌曲和播放位置可恢复；系统媒体键与页面控件必须映射到同一状态真值。
- 媒体结束自动切歌、错误重试和组件卸载清理必须有真实音频浏览器回归；歌词质量错误与音频错误分开呈现。
- 正式 Provider 合法音频 URL 仍为独立前置条件，测试只使用本地 WAV fixture，不接入私有音频接口。

### Phase 8 验收追加

- 质量报告必须区分 `word_timed`、`line_timed`、`unsynced`、`invalid`，并提供稳定诊断码和上下文。
- 逐行 timing 可降级播放但不得声明逐字能力；raw 或缺失显式时间戳的文本不得伪造同步播放。
- 检测行倒序/重叠、空文本、逐字倒序/重叠/越界/零时长、标注越界；阻断性问题影响可信度和可播放判断。
- 真实 QQ/QRC、普通 LRC、raw、多语和异常 timing 自动化通过，现有确定性播放状态机不得回归。
- 本阶段先在前端 normalization 后建立稳定诊断契约，不立即升级 Unified JSON Schema；协议升级需另行评审兼容性。

### 关联 plan
- `.plan/2026-07-28-qq-music-lyric-rendering-benchmark.md`

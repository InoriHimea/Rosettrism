# 2026-06-06 六大版本完成度複查報告

## 複查結論

前述六項優化任務目前已具備可追蹤的合併成果，整體完成度良好：後端可觀測性、AI 優選閉環、Dashboard Token、前端結構化、開發工作流與 Unified JSON Schema 都已落到程式碼或文件中。此次複查未發現需要立即阻斷使用的重大缺陷；主要剩餘空間集中在「更完整的自動化驗證」、「更細的權限與安全模型」、「資料庫維運能力」、「前端視覺系統深化」與「客戶端契約治理」。

## 六大版本完成度矩陣

| 大版本 | 原始優化主題 | 目前狀態 | 主要落點 | 複查判斷 |
|---|---|---:|---|---|
| v1.1.0 | AI 優選可追蹤化 | 已完成 | `src/service.rs`、`src/cache.rs`、`src/server.rs`、`frontend/src/views/InspectorView.jsx` | 已能記錄 AI 評分、候選摘要 hash、最佳候選與理由，並透過 API / Inspector 呈現。 |
| v1.2.0 | Dashboard Server Token | 已完成 | `frontend/src/api/client.js`、`frontend/src/views/SettingsView.jsx`、`src/server.rs` | 已集中處理 token header、sessionStorage 與 401 JSON 錯誤。 |
| v1.3.0 | `fetch_runs` 可觀測性 | 已完成 | `src/cache.rs`、`src/service.rs`、`src/server.rs`、`frontend/src/views/OverviewView.jsx`、`frontend/src/views/CacheView.jsx` | 已能記錄多種 fetch/search 任務、狀態分布與近期任務。 |
| v1.4.0 | 前端拆分與現代化設計基礎 | 已完成 | `frontend/src/views/`、`frontend/src/hooks/`、`frontend/src/api/client.js`、`frontend/src/styles/` | `App.jsx` 已由大型單檔拆成 views/hooks/api/styles；後續可在此基礎上深化設計系統。 |
| v1.5.0 | requirement / `.plan` 工作流 | 已完成 | `.plan/README.md`、`.plan/TEMPLATE.md`、`README-zh.md`、`requirement.md` | 已有模板與流程說明；本次補齊 v1.2.0-v1.6.0 的需求歷史，降低回溯斷點。 |
| v1.6.0 | Unified JSON Schema 與相容性策略 | 已完成 | `schema/unified-lyric.schema.json`、`docs/unified-json.md`、`tests/unified_schema.rs`、fixtures | 已加入 `schema_version`、schema 文件、fixtures 驗證與 README 說明。 |

## 仍可進一步優化的方向

### 短期優化（建議下一批 patch / minor）

1. **補強 API 端到端測試**：目前多數成果已有單元或 schema 測試，但 `/api/cache/:id`、`/api/runs`、token 401/成功路徑、AI score 輸出可再增加 server handler 或 HTTP integration 測試。
2. **統一錯誤碼與前端錯誤 UX**：目前可讀錯誤已改善，後續可建立 `{ code, message, details, retryable }` 錯誤格式，讓 Dashboard 能依錯誤類型顯示重試、輸入 token 或調整來源建議。
3. **補上 plan / requirement 檢查腳本**：新增輕量腳本或 CI job，偵測功能檔變更時是否同步更新 `requirement.md` 或 `.plan/`，避免流程只停留在人工約定。
4. **AI score 隱私遮罩**：AI 設定與 score 記錄已有 base URL / model，建議確認不會保存 API key；未來若記錄 prompt 或 provider 原文摘要，應加上遮罩與大小限制。

### 中期優化（建議獨立大版本）

1. **資料庫維運工具**：加入 cache / fetch_runs / ai_scores 的 prune、export、vacuum、migration status CLI，讓長期部署更容易維護。
2. **Provider 健康度統計**：以 `fetch_runs` 為基礎計算 provider 成功率、平均耗時、警告率與最近錯誤，Dashboard Overview 可呈現來源健康燈號。
3. **AI 評分回放與比較**：保存候選摘要與模型設定 hash 後，可支援同一查詢以不同模型或 prompt 重跑評分，建立品質回歸檢查。
4. **Schema 相容性治理**：新增 schema changelog、fixture golden snapshots 與主/次版本升級規則，讓客戶端能更可靠地升級。

### 長期優化（產品化 / 生態化）

1. **更科技感的視覺層**：在現有 tokens/layout/components 拆分基礎上，導入可切換 theme、動態頻譜背景、glassmorphism panel、neon focus ring 與 karaoke motion preset。
2. **插件式 Provider / Decoder**：把 provider metadata、rate-limit、健康度與能力宣告抽象化，未來可用 plugin 方式加入新來源或私有來源。
3. **多客戶端契約套件**：由 JSON Schema 產生 TypeScript / Kotlin / Swift 型別，降低客戶端解析成本。
4. **部署安全模型**：支援 token rotation、read-only token、admin token、CORS allowlist 與反向代理部署範本。

## 本次複查中的即時修正

- 修正 `tests/cli.rs` 的 `decode_ignores_cookie_file` fixture 路徑組合方式，改用 `PathBuf::from("tests").join("fixtures").join("sample.qrc")`，避免 Linux/macOS 將 Windows 反斜線視為一般檔名字元而導致測試找不到 fixture。
- 測試環境存在 HTTP/HTTPS proxy；執行 localhost wiremock 測試時需明確設定 `NO_PROXY=127.0.0.1,localhost` 與 `no_proxy=127.0.0.1,localhost`，避免 mock server 請求被代理攔截成 403。

## 建議下一步

建議下一個開發批次優先選擇「API 端到端測試 + 統一錯誤格式 + provider 健康度統計」三件事。這三項可以直接提高已完成六大版本的可靠性與可觀測性，而且不需要立即重做前端視覺或資料模型。

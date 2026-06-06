# 2026-06-06 — README 開源格式與 AGPLv3 授權同步

## 背景

- 使用者確認前一輪計畫已完成，並詢問目前專案是否仍需要功能迭代與優化工作。
- 本次要求同步將 README 調整為常見開源專案格式風格，並確認專案授權為 AGPLv3。
- 關聯需求：`requirement.md` v1.8.0。

## 目標

- [x] README 採用常見開源專案結構：簡介、功能、安裝、快速開始、文件、路線圖、貢獻、授權。
- [x] README / README-zh 明確回答目前仍建議做功能迭代與優化，並按短中長期列出方向。
- [x] Cargo / frontend package metadata 與 LICENSE / README 保持 AGPLv3 一致。
- [x] 補充需求歷史，讓本次文件與授權整理可追蹤。

## 非目標

- 本次不修改 CLI、Server API、Provider、Decoder 或 Dashboard runtime 行為。
- 本次不升級版本號、不建立 tag、不推送遠端。
- 本次不新增新的測試 fixture 或 schema 欄位。

## 風險

- README 與實作不一致：以現有 CLI、server、schema 與完成度報告為依據整理。
- 授權 metadata 與 LICENSE 不一致：同步更新 `Cargo.toml`、`frontend/package.json` 與 README 授權段落。
- 文件重寫造成資訊遺漏：保留原有 build、CLI、server、cache、annotations、sources、verification 等核心內容。

## 階段列表

### Phase 1 — 盤點與設計

#### Task checklist

- [x] 檢查現有 README / README-zh 結構與內容。
- [x] 檢查 LICENSE 與 package metadata 授權欄位。
- [x] 確認後續迭代建議來源。

#### 驗收條件

- [x] README 改版方向明確。
- [x] 授權不一致點已定位。

#### 測試紀錄

- `sed -n '1,260p' README.md README-zh.md Cargo.toml frontend/package.json LICENSE` 已完成人工盤點。

#### 完成狀態

- [x] 已完成

### Phase 2 — 文件與授權 metadata 更新

#### Task checklist

- [x] 重寫 `README.md` 為開源專案常見格式。
- [x] 重寫 `README-zh.md` 為中文開源專案常見格式。
- [x] 將 Rust crate license metadata 改為 AGPLv3。
- [x] 將 dashboard package metadata 補上 AGPLv3。
- [x] 更新 `requirement.md` 追加本次需求。

#### 驗收條件

- [x] README 中明確列出後續功能迭代與優化方向。
- [x] 授權欄位與 LICENSE 一致。

#### 測試紀錄

- 文件與 metadata 更新已完成，詳見整體測試紀錄。

#### 完成狀態

- [x] 已完成

### Phase 3 — 驗收與收尾

#### Task checklist

- [x] 執行格式與測試檢查。
- [x] 檢查 git diff 與工作區狀態。
- [x] 建立提交並準備 PR 訊息。

#### 驗收條件

- [x] 必要檢查通過或有明確說明。
- [x] 文件、metadata 與需求歷史已同步。

#### 測試紀錄

- 等待最終執行後填入整體測試紀錄。

#### 完成狀態

- [x] 已完成

## 整體驗收條件

- [x] README / README-zh 已更新為常見開源專案格式。
- [x] README 已回答仍需功能迭代與優化，並列出優先方向。
- [x] AGPLv3 授權已在 metadata 與文件中同步。
- [x] `requirement.md` 已追加本次需求。

## 整體測試紀錄

| 日期 | 指令 / 檢查 | 結果 | 備註 |
|------|-------------|------|------|
| 2026-06-06 | `cargo fmt --check` | 通過 | Rust 格式檢查。 |
| 2026-06-06 | `NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost cargo test` | 通過 | Rust 單元、整合與 doc tests。 |
| 2026-06-06 | `npm run build`（於 `frontend/`） | 通過 | Vite production build；npm 顯示既有 `http-proxy` env config warning。 |

## 整體完成狀態

- [x] 已完成

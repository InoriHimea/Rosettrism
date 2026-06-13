# 2026-06-06 — Plan / Requirement 一致性檢查

## 背景

- 使用者要求在進入下一階段開發前，先為已修改優化版本建立 tag，再繼續後續計畫。
- 已於本次開始時在目前 HEAD 建立 `v4.8.13` annotated tag，作為上一輪已修改優化版本的切點。
- 依完成度複查短期優化建議，下一個可低風險落地的項目是補上 plan / requirement 一致性檢查，避免流程只停留於人工約定。
- 關聯需求：`requirement.md` v1.9.0。

## 目標

- [x] 新增輕量腳本，偵測 feature-sensitive 檔案變更時是否同步更新 `requirement.md` 或 `.plan/`。
- [x] 支援 staged 與指定 base ref 兩種常見檢查情境。
- [x] 在 README / README-zh 的開發檢查中補充使用方式。

## 非目標

- 本次不導入新的 CI 平台設定或 Git hooks 自動安裝。
- 本次不改動 CLI、Server API、Provider 或 Dashboard runtime 行為。
- 本次不新增資料庫 schema 或前端畫面。

## 風險

- 誤判文件型變更：腳本只將 `src/`、`tests/`、`schema/`、`scripts/`、前端源碼與 package metadata 等列為 feature-sensitive，降低純文件變更誤報。
- 分支基準不同：提供 `--base <ref>` 讓 CI 或維護者指定比較基準；未指定時預設比較 `HEAD~1`。
- 流程阻力：腳本只檢查是否更新 `requirement.md` 或 `.plan/`，不限制內容格式，先以輕量守門降低成本。

## 階段列表

### Phase 1 — 需求釐清與 tag 切點

#### Task checklist

- [x] 確認目前工作區無未提交變更。
- [x] 建立 `v4.8.13` annotated tag 作為已修改優化版本切點。
- [x] 選定短期優化項目：plan / requirement 一致性檢查。

#### 驗收條件

- [x] tag 已建立於進入本次開發前的 HEAD。
- [x] 本次變更範圍已記錄。

#### 測試紀錄

- `git tag -a v4.8.13 -m "v4.8.13 optimized release"` 已執行成功。

#### 完成狀態

- [x] 已完成

### Phase 2 — 腳本與文件實作

#### Task checklist

- [x] 新增 `scripts/check-plan-requirement.sh`。
- [x] 更新 README / README-zh 的 recommended checks。
- [x] 更新 `requirement.md` 追加 v1.9.0。

#### 驗收條件

- [x] 腳本能在本次 feature-sensitive 變更搭配 plan / requirement 更新時通過。
- [x] 文件包含可複製執行的命令。

#### 測試紀錄

- 詳見整體測試紀錄。

#### 完成狀態

- [x] 已完成

### Phase 3 — 驗收與收尾

#### Task checklist

- [x] 執行腳本自檢。
- [x] 執行 Rust 格式檢查與測試。
- [x] 檢查 git diff 與工作區狀態。

#### 驗收條件

- [x] 所有必要檢查已通過或有明確說明。
- [x] 文件已同步更新。

#### 測試紀錄

- 詳見整體測試紀錄。

#### 完成狀態

- [x] 已完成

## 整體驗收條件

- [x] 需求目標已完成
- [x] 測試紀錄完整
- [x] 相關文件已同步
- [x] `requirement.md` 已追加或確認不需追加

## 整體測試紀錄

| 日期 | 指令 / 檢查 | 結果 | 備註 |
|------|-------------|------|------|
| 2026-06-06 | `scripts/check-plan-requirement.sh --base HEAD` | 通過 | 本次變更包含 plan / requirement 更新 |
| 2026-06-06 | `cargo fmt --check` | 通過 | 無 Rust 格式變更 |
| 2026-06-06 | `NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost cargo test` | 通過 | localhost mock 避免 proxy |
| 2026-06-06 | `cd frontend && npm run build` | 通過 | npm 顯示 http-proxy env config deprecation warning，build 成功 |

## 整體完成狀態

- [ ] 未開始
- [ ] 進行中
- [x] 已完成
- [ ] 已取消

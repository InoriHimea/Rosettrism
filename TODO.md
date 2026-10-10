# TODO

未完成 / 受阻 / 部分完成事项清单。完成后勾选或移除。对应搁置特性见 `feature.md`。

## 进行中

- [ ] v5.0.0 收尾：server 冒烟（`/api/health` `/api/stats` `/api/cache`）+ `cargo build --release --locked` 终验，完成后更新 `.plan/2026-10-10-sqlite-async-and-quality.md` Phase 3 状态。
- [ ] v5.0.0 提交与发布：3 个计划内 commit（① SQLite 异步化 + panic 策略 + WAL；② service.rs 拆分；③ 新测试 + 流程文件 + 版本号），tag `v5.0.0`，多远端推送 + GitHub Actions 检查。用户尚未要求提交。

## 待办（明确计划、尚未开工）

- [ ] `server.rs`（约 2312 行）拆分：v5.0.0 只拆了 service.rs；server.rs 的 handlers 按资源域（cache/unified/runs/ai/静态资源）拆分另行立项。
- [ ] `cache.rs`（约 1721 行）拆分：v5.0.0 叠加了 async 改造，避免同轮再拆；后续可按表域（upstream/unified/fetch_runs/ai_scores/维护操作）分文件。
- [ ] Git stale 远程分支清理：约 130 个 `lost_world/worktree-agent-*` 远程分支（历史 agent 工作树残留），清理需用户确认后执行 `git push lost_world --delete`。
- [ ] master 领先 lost_world/master 多个提交未推送（与下次发布一起推）。

## 受阻 / 环境限制

- [ ] Playwright Chromium CDN 403：`frontend npm test` 在无 Chromium 且 CDN 被墙的环境失败（v1.8.0 已记录）；spec 本身在 Chromium 存在时全过。解法待定（镜像源或手动安装）。
- [ ] Phase 9 Firefox runner 恢复异常未修：Edge/Chrome 过 4/4，Firefox 需 `scripts/cleanup-firefox-zombies.mjs` 兜底。
- [ ] Phase 7 真实 Provider 音频 URL 属外部前置条件，本地仅 WAV fixture 覆盖（v2.3.0 记录）。
- [ ] 本机 F: 盘 incremental compilation hard-link warning（文件系统不支持硬链接，cargo 回退 copy）：无功能影响，可通过 `CARGO_INCREMENTAL=0` 或迁移缓存目录消除。

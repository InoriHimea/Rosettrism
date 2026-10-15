# TODO

未完成 / 受阻 / 部分完成事项清单。完成后勾选或移除。对应搁置特性见 `feature.md`。

## 进行中

（空）

## 待办（明确计划、尚未开工）

（空）

## 受阻 / 环境限制

- [ ] Playwright Chromium CDN 403：`frontend npm test` 在无 Chromium 且 CDN 被墙的环境失败（v1.8.0 已记录）；spec 本身在 Chromium 存在时全过。解法待定（镜像源或手动安装）。
- [ ] Phase 9 Firefox runner 恢复异常未修：Edge/Chrome 过 4/4，Firefox 需 `scripts/cleanup-firefox-zombies.mjs` 兜底。
- [ ] Phase 7 真实 Provider 音频 URL 属外部前置条件，本地仅 WAV fixture 覆盖（v2.3.0 记录）。
- [ ] 本机 F: 盘 incremental compilation hard-link warning（文件系统不支持硬链接，cargo 回退 copy）：无功能影响，可通过 `CARGO_INCREMENTAL=0` 或迁移缓存目录消除。

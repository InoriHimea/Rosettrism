# Feature 备忘

提出、搁置或刻意延后的特性与改进（当前不在做）。上线后将条目移除或标记完成。当前进行中的未完成事项见 `TODO.md`。

## 已提出 / 搁置

- **前端 hooks / views 组件测试**：`frontend/src/hooks/`（5 个）与 `views/`（5 个）零隔离测试，仅 Playwright E2E 间接覆盖。需先引入组件测试基础设施（Vitest + Testing Library 或 Playwright Component），v5.0.0 明确列为非目标，待基础设施决策后立项。
- **server.rs / cache.rs 文件拆分**：v5.0.0 仅拆 service.rs；server.rs（~2312 行）与 cache.rs（~1721 行）的按域拆分计划见 `TODO.md` 待办区。
- **SQLite 异步驱动迁移（sqlx 等）**：评估后搁置——spawn_blocking + std Mutex 已消除阻塞（锁守卫不跨 await），迁移收益不抵依赖与改造成本。若未来出现跨进程高并发写瓶颈再重评。
- **provider 插件化 M2–M4**：M1 注册表化已完成（v3.1.0）；M2 manifest 驱动配置、M3 外部 provider 插件（WASM 优先）、M4 外部 decoder 插件按 `docs/provider-plugin-manifest.md` 路线图推进，独立立项。
- **Playwright Chromium 镜像/离线安装方案**：CDN 403 环境问题的长期解法（`PLAYWRIGHT_DOWNLOAD_HOST` 镜像或预装缓存），待环境确认。

## 已完成（从搁置转正并上线）

- （空——后续条目上线后从上方移入此处记录版本号，超过 3 条时清理归档）

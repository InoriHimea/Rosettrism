# 助唱标注 Playwright 实测报告

## 范围

- 修复目标：QQ 助唱标注在逐字歌词上的位置错乱。
- 运行表面：前端真实页面，通过搜索页、候选详情、获取 JSON、播放按钮进入歌词播放 UI。
- 运行方式：Vite dev server + Playwright Chromium（非 headless）+ API route mock 注入 QQ 风格搜索和歌词详情响应。
- 变更文件：`frontend/src/lyricPlayback.js`、`frontend/src/styles.css`，并由 `npm run build` 更新 `frontend/dist/`。

## 复现结论

修复前，标注归属算法使用较宽的时间窗口把同一个标注挂到多个相邻字上；渲染层按 `visibleWordAnnotations` 只展示第一个命中的字，导致例如第 2 个字的重音被画到第 1 个字附近。

证据截图：`frontend/playwright-artifacts/annotation-before-fix.png`

关键观察：

- 标注时间 `8650ms` 应落在第 3 个词 `c` 的起点附近（行起点 `7200ms` + 词 offset `1300ms` = `8500ms`）。
- 修复前标注显示在更靠前的词附近，因为多个词被同时命中后只展示第一个。

## 修复策略

1. 每个助唱标注只选择一个最佳锚点词，而不是挂到所有时间窗口相交的词。
2. 锚点优先使用文本匹配；否则按标注起点到词起点/词中心的最小距离选择。
3. 移除换气标注的负向位移，确保标注层以被锚定词的中心线居中。
4. 调整重音标注垂直位移，使点和标签不再过度下坠。

## 测试用例

### TC-01：英文标签标注锚定到目标词

**数据**

- 行起点：`7200ms`
- 词：`a(7200ms)`、`b(7850ms)`、`c(8500ms)`、`d(9150ms)`、`e(9800ms)`、`f(10450ms)`
- 标注：
  - `breath` at `7600ms`
  - `stress` at `8650ms`
  - `long_tone` at `10200ms`

**步骤**

1. 打开前端页面。
2. 点击“获取”。
3. 输入 title/artist 并搜索。
4. 点击候选结果。
5. 点击“获取 JSON”。
6. 点击播放。
7. 等待到第一句 active，截图并读取 `.lyric-annotation-mark` 与 `.lyric-word` 的 bounding box。

**期望**

- `breath` 居中到第 1 个词。
- `stress` 居中到第 3 个词。
- `long_tone` 居中到第 5 个词。

**实际**

- 三个标注的中心点均与目标词中心对齐，delta 为 `0px`。
- 截图：`frontend/playwright-artifacts/annotation-after-fix-active.png`

### TC-02：QQ 风格无 text 字段标注使用默认中文标签并居中

**数据**

- 标注只提供 `annotation_type/start_ms/duration_ms`，不提供 `text`。
- 类型：`breath`、`stress`、`long_tone`。

**步骤**

1. 使用同一真实 UI 流程进入播放。
2. 等待 active 行。
3. 截图并采集标注和词的 bounding box。

**期望**

- 默认标签显示为“换气 / 重音 / 长音”。
- `V`、`·`、长音胶囊分别位于被锚定词上方，水平居中。

**实际**

- `换气V` → 第 1 个词，delta `0px`。
- `重音·` → 第 2 个词，delta `0px`。
- `长音` → 第 5 个词，delta `0px`。
- 截图：`frontend/playwright-artifacts/annotation-after-fix-symbols.png`

### TC-03：滚动到下一行后旧行标注不跟随 active 行错位

**步骤**

1. 从 TC-02 继续播放。
2. 等待滚动到下一句歌词。
3. 截图并读取 active 行标注数量。

**期望**

- active 行切换到下一句。
- 下一句没有助唱标注时，active 行不显示旧标注。

**实际**

- active 行切换到 `next line`。
- active 行标注数量为 `0`。
- 截图：`frontend/playwright-artifacts/annotation-after-fix-scrolled.png`

## 验证命令

```powershell
Set-Location "f:\VsCodeProject\lrc-decode\frontend"
npm run dev
# Playwright 脚本通过 route mock 驱动真实 UI 流程并截图
npm run build
```

## 结果

- Playwright 实测：通过。
- 生产构建：通过。
- 当前剩余限制：测试使用 API route mock 注入 QQ 风格 JSON，未依赖真实后端网络；UI 入口、详情弹窗、播放控件和歌词渲染均走真实前端页面流程。

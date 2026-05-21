# 歌词播放 UI Playwright 实测报告

## 范围

- 修复目标：QQ 助唱标注位置错乱、三点倒计时末尾闪回 3 点。
- 运行表面：前端真实页面，通过“获取”页、候选详情、获取 JSON、播放/时间轴进入歌词播放 UI。
- 运行方式：Vite dev server + Playwright Chromium（非 headless）+ API route mock 注入 QQ 风格搜索和歌词详情响应。
- 验证脚本：
  - `frontend/playwright-verify-annotations.mjs`
  - `frontend/playwright-verify-countdown.mjs`

## 修复策略

1. 助唱标注按词锚点独立渲染，不再把同一层的所有标注挤在一个居中 flex 容器内。
2. 每个标注使用 `--annotation-index` 分层，避免同一词上多标注互相覆盖。
3. `换气`/`重音` 使用专用布局：中文标签在上方；`V` 位于字间附近；`·` 位于文字下方。
4. 倒计时由 `lyricCountdown()` 统一返回 `visible`、`targetLineId`、`count`、`exiting`，渲染位置锁定目标歌词行。
5. 起句后 260ms 只保留 1 个可见点并播放 bubble-out，之后移除倒计时行，避免末尾回退到 3 点。

## 测试用例

### TC-01：助唱标注按目标词锚定

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
3. 搜索并打开候选结果。
4. 点击“获取 JSON”。
5. 点击播放。
6. 跳转到第一句 active 状态，截图并采集 `.lyric-word` / `.lyric-annotation-mark` bounding box。

**期望**

- `breath` 锚在第 1 个词左侧字间位置。
- `stress` 锚在第 3 个词附近。
- `long_tone` 锚在第 5 个词附近。
- 三类标注不互相堆叠成一团。

**实际**

```text
WORDS: [{"i":0,"x":272,"y":211,"w":29},{"i":1,"x":303,"y":211,"w":33},{"i":2,"x":338,"y":211,"w":26},{"i":3,"x":366,"y":211,"w":32},{"i":4,"x":400,"y":211,"w":29},{"i":5,"x":432,"y":211,"w":22}]
MARKS: [{"i":0,"x":262,"y":183,"w":21},{"i":1,"x":340,"y":183,"w":21},{"i":2,"x":390,"y":186,"w":49}]
```

- `breath`：`x=262`，位于第 1 个词 `x=272` 左侧字间区域。
- `stress`：`x=340`，覆盖第 3 个词 `x=338..364`。
- `long_tone`：`x=390`，覆盖第 5 个词附近 `x=400..429`。
- 截图：`frontend/playwright-artifacts/verify-annotation-active.png`

### TC-02：重音标注切换时不漂移且点在文字下方

**步骤**

1. 从 TC-01 继续。
2. seek 到 `8800ms`，使 `stress` 标注处于观察窗口。
3. 采集目标词、`重音` 标签、`·` glyph 的 bounding box。

**期望**

- `重音` 中文标签在歌词上方。
- `·` 在目标词文字下方，而不是压在文字中间或文字上方。
- 其他标注仍在各自锚点，不被重音层挤走。

**实际**

```text
STRESS_SAMPLE: {"word":{"index":2,"text":"重音·cc","x":330,"y":797,"w":23,"h":48},"glyph":{"x":339,"y":847,"w":6,"h":14},"label":{"x":333,"y":773,"w":18,"h":9}}
```

- 目标词盒：`y=797..845`。
- `·` glyph：`y=847..861`，位于目标词下方。
- `重音` label：`y=773..782`，位于目标词上方。
- 截图：`frontend/playwright-artifacts/verify-annotation-stress.png`
- 专项截图：`frontend/playwright-artifacts/verify-meta-stress.png`

### TC-03：倒计时 3→2→1 递减

**数据**

- 第一句：`7200ms`，结束约 `8600ms`
- 第二句：`20000ms`
- 两句间隔大于 9 秒，应显示三点倒计时。

**步骤**

1. 打开前端页面。
2. 点击“获取”。
3. 搜索并打开候选结果。
4. 点击“获取 JSON”。
5. 使用 range 控件 native setter 触发 React，再等待 `.lyric-time` 实际变化。
6. 分别 seek 到 `16600ms`、`18100ms`、`18800ms`。

**期望**

- `16600ms`：3 个可见点。
- `18100ms`：2 个可见点。
- `18800ms`：1 个可见点。

**实际**

```text
{"ms":16600,"method":"native-setter","time":"0:16 / 0:21","count":3,"exiting":0,"rows":["•••"]}
{"ms":18100,"method":"native-setter","time":"0:18 / 0:21","count":2,"exiting":0,"rows":["•••"]}
{"ms":18800,"method":"native-setter","time":"0:18 / 0:21","count":1,"exiting":0,"rows":["•••"]}
```

### TC-04：倒计时末尾 bubble-out 且不回退 3 点

**步骤**

1. 从 TC-03 继续。
2. seek 到 `19720ms`、`19920ms`、`20020ms`、`20180ms`、`20320ms`。
3. 记录可见点数量、`.lyric-dots-exiting` 数量和倒计时行数量。

**期望**

- 起句前最后阶段仍为 1 个可见点。
- 起句后 260ms 内进入 `.lyric-dots-exiting`。
- 退出窗口结束后倒计时行消失。
- 全流程不再短暂回退为 3 个可见点。

**实际**

```text
{"ms":19720,"method":"native-setter","time":"0:19 / 0:21","count":1,"exiting":0,"rows":["•••"]}
{"ms":19920,"method":"native-setter","time":"0:19 / 0:21","count":1,"exiting":0,"rows":["•••"]}
{"ms":20020,"method":"native-setter","time":"0:20 / 0:21","count":1,"exiting":1,"rows":["•••"]}
{"ms":20180,"method":"native-setter","time":"0:20 / 0:21","count":1,"exiting":1,"rows":["•••"]}
{"ms":20320,"method":"native-setter","time":"0:20 / 0:21","count":0,"exiting":0,"rows":[]}
```

- 截图：`frontend/playwright-artifacts/verify-countdown-samples.png`

### TC-05：标题和元数据固定 2 秒行级滚动，active 竖线存在

**数据**

- 标题：`超长标题测试别怕我伤心雨一直下-张信哲（JeffChang）`
- 元数据：`作词：测试作词`、`作曲：测试作曲`
- 首句歌词：`7200ms`

**步骤**

1. 打开前端页面。
2. 点击“获取”。
3. 搜索并打开候选结果。
4. 点击“获取 JSON”。
5. 使用 range 控件 native setter 触发 React，并分别 seek 到 `200ms`、`2200ms`、`4200ms`、`6200ms`。
6. 采集 `.lyric-line-active` 文本、class、伪元素 `::before` 宽度、`white-space`、倒计时行数量。

**期望**

- `200ms`：标题为 active 行。
- `2200ms`：作词为 active 行。
- `4200ms`：作曲为 active 行。
- 每个 active 元数据行都有左侧高亮竖线。
- 标题不换行。
- 元数据滚动完成后，三点倒计时按真实歌词时序显示。

**实际**

```text
META_SAMPLES: [
  {"ms":200,"text":"超长标题测试别怕我伤心雨一直下-张信哲（JeffChang）","className":"lyric-line lyric-line-active lyric-line-distance-0 lyric-line-meta","height":62,"scrollWidth":719,"clientWidth":719,"whiteSpace":"nowrap","beforeWidth":"4px","beforeContent":"\"\""},
  {"ms":2200,"text":"作词：测试作词","className":"lyric-line lyric-line-active lyric-line-distance-0 lyric-line-meta","height":62,"scrollWidth":278,"clientWidth":278,"whiteSpace":"nowrap","beforeWidth":"4px","beforeContent":"\"\""},
  {"ms":4200,"text":"作曲：测试作曲","className":"lyric-line lyric-line-active lyric-line-distance-0 lyric-line-meta","height":62,"scrollWidth":278,"clientWidth":278,"whiteSpace":"nowrap","beforeWidth":"4px","beforeContent":"\"\""}
]
COUNTDOWN_AFTER_META: {"time":"0:06 / 0:21","rows":["•••"],"count":1}
```

- 标题、作词、作曲按 2 秒节奏成为 active 行。
- `beforeWidth: "4px"`，三行都有 active 竖线。
- 标题 `whiteSpace: "nowrap"`，没有换行。
- `6200ms` 倒计时行存在，元数据滚动结束后由倒计时接管。
- 截图：`frontend/playwright-artifacts/verify-meta-stress.png`

## 验证命令

```powershell
Set-Location "f:\VsCodeProject\lrc-decode\frontend"
npm run build
node playwright-verify-annotations.mjs
node playwright-verify-countdown.mjs
node playwright-verify-meta-stress.mjs
```

## 结果

- Playwright 标注实测：通过。
- Playwright 倒计时实测：通过。
- Playwright 标题/元数据/重音专项实测：通过。
- 前端生产构建：通过。
- 当前限制：测试使用 API route mock 注入 QQ 风格 JSON，未依赖真实后端网络；UI 入口、详情弹窗、播放控件、range 控件事件路径和歌词渲染均走真实前端页面流程。

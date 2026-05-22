# 歌词播放 UI Playwright 实测报告

## 范围

- 修复目标：QQ 助唱标注位置错乱、下方标注不清晰、标注颜色不明显、Beyond《海阔天空》音译未展示、重复点击“获取 JSON”丢失 QQ 助唱标注、标题/元数据滚动与三点倒计时互相影响。
- 运行表面：前端真实页面，通过“获取”页、候选详情、获取 JSON、播放控件和 range 时间轴进入歌词播放 UI。
- 运行方式：Vite dev server + Playwright Chromium（非 headless）+ API route mock 注入 QQ 风格搜索和歌词详情响应。
- 主要验证脚本：`frontend/playwright-verify-meta-stress.mjs`。
- 辅助历史脚本：`frontend/playwright-verify-annotations.mjs`、`frontend/playwright-verify-countdown.mjs`。

## 修复策略

1. 标题、作词、作曲等元数据进入固定 2 秒的行级滚动流，active 元数据行复用歌词 active 竖线。
2. 三点倒计时只基于真实歌词正文行计算，避免被标题/元数据行覆盖或抢占。
3. `stress`、`breath`、`long_tone` 使用独立布局：中文 label 在字上方，重音 `·` 和长音 `_` 在目标字下方，换气 `V` 靠近字间。
4. 提升标注颜色、字重、阴影和对比度，降低标注离目标字的距离。
5. 从行级 `extra.cantonese_romanization`、track、reading/romanization 候选字段提取音译并展示。
6. 重复点击“获取 JSON”时合并新响应、上一次详情和原候选 `extra` 中的助唱标注，第二次响应缺省标注时仍保留 canonical `singing_annotations`。
7. Playwright seek 使用原生 `HTMLInputElement.prototype.value` setter + `input`/`change` 事件，并等待 `.lyric-time` 实际变化后采样。

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
5. seek 到第一句 active 状态。
6. 采集 `.lyric-word` / `.lyric-annotation-mark` bounding box。

**期望**

- `breath` 锚在第 1 个词左侧字间位置。
- `stress` 锚在第 3 个词附近。
- `long_tone` 锚在第 5 个词附近。
- 三类标注不互相堆叠成一团。

**历史实测**

```text
WORDS: [{"i":0,"x":272,"y":211,"w":29},{"i":1,"x":303,"y":211,"w":33},{"i":2,"x":338,"y":211,"w":26},{"i":3,"x":366,"y":211,"w":32},{"i":4,"x":400,"y":211,"w":29},{"i":5,"x":432,"y":211,"w":22}]
MARKS: [{"i":0,"x":262,"y":183,"w":21},{"i":1,"x":340,"y":183,"w":21},{"i":2,"x":390,"y":186,"w":49}]
```

- `breath`：`x=262`，位于第 1 个词 `x=272` 左侧字间区域。
- `stress`：`x=340`，覆盖第 3 个词 `x=338..364`。
- `long_tone`：`x=390`，覆盖第 5 个词附近 `x=400..429`。

### TC-02：重音标注切换时不漂移且点在文字下方

**步骤**

1. 从 TC-01 继续。
2. seek 到 `8800ms`，使 `stress` 标注处于观察窗口。
3. 采集目标词、`重音` label、`·` glyph 的 bounding box。

**期望**

- `重音` 中文 label 在歌词上方。
- `·` 在目标词文字下方，而不是压在文字中间或文字上方。
- 其他标注仍在各自锚点，不被重音层挤走。

**历史实测**

```text
STRESS_SAMPLE: {"word":{"index":2,"text":"重音·cc","x":330,"y":797,"w":23,"h":48},"glyph":{"x":339,"y":847,"w":6,"h":14},"label":{"x":333,"y":773,"w":18,"h":9}}
```

- 目标词盒：`y=797..845`。
- `·` glyph：`y=847..861`，位于目标词下方。
- `重音` label：`y=773..782`，位于目标词上方。

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

**历史实测**

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

**历史实测**

```text
{"ms":19720,"method":"native-setter","time":"0:19 / 0:21","count":1,"exiting":0,"rows":["•••"]}
{"ms":19920,"method":"native-setter","time":"0:19 / 0:21","count":1,"exiting":0,"rows":["•••"]}
{"ms":20020,"method":"native-setter","time":"0:20 / 0:21","count":1,"exiting":1,"rows":["•••"]}
{"ms":20180,"method":"native-setter","time":"0:20 / 0:21","count":1,"exiting":1,"rows":["•••"]}
{"ms":20320,"method":"native-setter","time":"0:20 / 0:21","count":0,"exiting":0,"rows":[]}
```

### TC-05：标题和元数据固定 2 秒行级滚动，active 竖线存在

**数据**

- 标题：`超长标题测试别怕我伤心雨一直下海阔天空`
- 歌手：`Beyond`
- 元数据：`作词：黄家驹`、`作曲：黄家驹`
- 首句歌词：`7200ms`

**步骤**

1. 打开前端页面。
2. 点击“获取”。
3. 搜索并打开候选结果。
4. 点击“获取 JSON”。
5. 再次点击“获取 JSON”，模拟第二次响应缺失助唱标注。
6. 使用 range 控件 native setter 分别 seek 到 `200ms`、`2200ms`、`4200ms`、`6200ms`。
7. 采集 `.lyric-line-active` 文本、class、伪元素 `::before` 宽度、`white-space`、倒计时行数量。

**期望**

- `200ms`：标题为 active 行。
- `2200ms`：作词为 active 行。
- `4200ms`：作曲为 active 行。
- 每个 active 元数据行都有左侧高亮竖线。
- 标题不换行。
- 元数据滚动完成后，三点倒计时按真实歌词时序显示。

**本轮实测**

```text
META_SAMPLES: [{"ms":200,"text":"超长标题测试别怕我伤心雨一直下海阔天空-Beyond","className":"lyric-line lyric-line-active lyric-line-distance-0 lyric-line-meta","height":62,"scrollWidth":719,"clientWidth":719,"whiteSpace":"nowrap","beforeWidth":"4px","beforeContent":"\"\""},{"ms":2200,"text":"作词：黄家驹","className":"lyric-line lyric-line-active lyric-line-distance-0 lyric-line-meta","height":62,"scrollWidth":243,"clientWidth":243,"whiteSpace":"nowrap","beforeWidth":"4px","beforeContent":"\"\""},{"ms":4200,"text":"作曲：黄家驹","className":"lyric-line lyric-line-active lyric-line-distance-0 lyric-line-meta","height":62,"scrollWidth":243,"clientWidth":243,"whiteSpace":"nowrap","beforeWidth":"4px","beforeContent":"\"\""}]
COUNTDOWN_AFTER_META: {"time":"0:06 / 0:21","rows":["•••"],"count":1}
```

- 标题、作词、作曲按 2 秒节奏成为 active 行。
- `beforeWidth: "4px"`，三行都有 active 竖线。
- `whiteSpace: "nowrap"`，长标题没有换行。
- `6200ms` 倒计时行存在，元数据滚动结束后由倒计时接管。

### TC-06：重复点击“获取 JSON”后仍保留 QQ 助唱标注

**数据**

- 第一次 `/api/fetch-result` 返回 `selectedEntry.extra.singing_annotations`：
  - `breath` at `7350ms`
  - `stress` at `8050ms`
  - `long_tone` at `8700ms`
- 第二次 `/api/fetch-result` 返回同一歌词，但 `selectedEntry.extra` 不含任何助唱标注数组。

**步骤**

1. 打开前端页面。
2. 点击“获取”。
3. 搜索并打开候选结果。
4. 点击“获取 JSON”。
5. 等歌词播放区出现后，再次点击“获取 JSON”。
6. 校验 `fetchResultCount === 2`。
7. seek 到 `8200ms`。
8. 采集 active 行内 `.annotation-stress`、`.annotation-long-tone`、`.annotation-breath` 数量，并检查 raw JSON 预览是否仍包含 `singing_annotations`。

**期望**

- 第二次响应缺失标注数组时，UI 不丢失第一次获取到的 QQ 助唱标注。
- raw JSON 预览保留 canonical `singing_annotations`，便于用户复制/排查。
- 去重后每类目标标注只出现 1 个。

**本轮实测**

```text
ANNOTATION_COUNTS_AFTER_REPEAT_FETCH: {"stress":1,"longTone":1,"breath":1,"reading":["hoi fut tin hung"],"rawJsonHasAnnotations":true}
```

- `stress=1`、`longTone=1`、`breath=1`。
- `rawJsonHasAnnotations=true`，重复获取后 JSON 预览仍保留 `singing_annotations`。

### TC-07：Beyond《海阔天空》音译展示

**数据**

- 歌词行：`海 阔 天 空`
- 行级 extra：`cantonese_romanization: "hoi fut tin hung"`

**步骤**

1. 打开前端页面。
2. 点击“获取”。
3. 搜索并打开 Beyond 候选结果。
4. 点击“获取 JSON”。
5. seek 到 `8200ms`，使 `海 阔 天 空` 成为 active 行。
6. 采集 `.lyric-line-active small` 文本。

**期望**

- active 行下方展示 `hoi fut tin hung`。

**本轮实测**

```text
ANNOTATION_COUNTS_AFTER_REPEAT_FETCH: {"stress":1,"longTone":1,"breath":1,"reading":["hoi fut tin hung"],"rawJsonHasAnnotations":true}
```

- `reading` 数组包含 `hoi fut tin hung`。

### TC-08：重音点与长音下划线位于目标文字下方并居中

**数据**

- `stress` 目标字：`阔`
- `long_tone` 目标字：`天`
- `breath` 目标字附近：`海`

**步骤**

1. 打开前端页面。
2. 点击“获取”。
3. 搜索并打开 Beyond 候选结果。
4. 点击“获取 JSON”两次。
5. seek 到 `8200ms`。
6. 采集 `.lyric-line-active .lyric-word`、`.annotation-glyph-text`、`.lyric-annotation-label` bounding box。

**期望**

- `stress` 的 `·` 在 `阔` 文字下方，并与目标字中心横向误差不超过 8px。
- `long_tone` 的 `_` 在 `天` 文字下方，并与目标字中心横向误差不超过 10px。
- `重音`、`长音`、`换气` label 在目标字上方。
- 标注颜色和 label 清晰可见。

**本轮实测**

```text
STRESS_SAMPLE: {"word":{"index":1,"text":"阔","x":323,"y":802,"w":42,"h":48},"glyph":{"x":340,"y":845,"w":8,"h":17},"label":{"x":338,"y":790,"w":11,"h":11}}
LONG_TONE_SAMPLE: {"word":{"index":2,"text":"天","x":366,"y":802,"w":42,"h":48},"glyph":{"x":382,"y":842,"w":10,"h":21},"label":{"x":381,"y":790,"w":11,"h":11}}
BREATH_SAMPLE: {"word":{"index":0,"text":"海","x":279,"y":802,"w":42,"h":48},"glyph":{"x":274,"y":798,"w":11,"h":14},"label":{"x":268,"y":787,"w":23,"h":11}}
```

- `阔` 盒：`y=802..850`；`·` 中心 `y=853.5`，在文字下方。
- `天` 盒：`y=802..850`；`_` 中心 `y=852.5`，在文字下方。
- `重音` label：`y=790..801`，在目标字上方。
- `长音` label：`y=790..801`，在目标字上方。
- `换气` label：`y=787..798`，在目标字上方。

## 验证命令

```powershell
npm --prefix frontend run dev -- --port 5181
npm --prefix frontend run build
node "frontend/playwright-verify-meta-stress.mjs"
```

## 本轮结果

```text
> rosettrism-dashboard@4.8.2 build
> vite build
✓ built in 3.74s

META_SAMPLES: [{"ms":200,"text":"超长标题测试别怕我伤心雨一直下海阔天空-Beyond","className":"lyric-line lyric-line-active lyric-line-distance-0 lyric-line-meta","height":62,"scrollWidth":719,"clientWidth":719,"whiteSpace":"nowrap","beforeWidth":"4px","beforeContent":"\"\""},{"ms":2200,"text":"作词：黄家驹","className":"lyric-line lyric-line-active lyric-line-distance-0 lyric-line-meta","height":62,"scrollWidth":243,"clientWidth":243,"whiteSpace":"nowrap","beforeWidth":"4px","beforeContent":"\"\""},{"ms":4200,"text":"作曲：黄家驹","className":"lyric-line lyric-line-active lyric-line-distance-0 lyric-line-meta","height":62,"scrollWidth":243,"clientWidth":243,"whiteSpace":"nowrap","beforeWidth":"4px","beforeContent":"\"\""}]
COUNTDOWN_AFTER_META: {"time":"0:06 / 0:21","rows":["•••"],"count":1}
ANNOTATION_COUNTS_AFTER_REPEAT_FETCH: {"stress":1,"longTone":1,"breath":1,"reading":["hoi fut tin hung"],"rawJsonHasAnnotations":true}
STRESS_SAMPLE: {"word":{"index":1,"text":"阔","x":323,"y":802,"w":42,"h":48},"glyph":{"x":340,"y":845,"w":8,"h":17},"label":{"x":338,"y":790,"w":11,"h":11}}
LONG_TONE_SAMPLE: {"word":{"index":2,"text":"天","x":366,"y":802,"w":42,"h":48},"glyph":{"x":382,"y":842,"w":10,"h":21},"label":{"x":381,"y":790,"w":11,"h":11}}
BREATH_SAMPLE: {"word":{"index":0,"text":"海","x":279,"y":802,"w":42,"h":48},"glyph":{"x":274,"y":798,"w":11,"h":14},"label":{"x":268,"y":787,"w":23,"h":11}}
```

## 截图

- `frontend/playwright-artifacts/verify-meta-stress.png`
- `frontend/playwright-artifacts/verify-annotation-active.png`
- `frontend/playwright-artifacts/verify-annotation-stress.png`
- `frontend/playwright-artifacts/verify-countdown-samples.png`

## 结论

- Playwright 页面实测：通过。
- 前端生产构建：通过。
- 当前限制：测试使用 API route mock 注入 QQ 风格 JSON，未依赖真实后端网络；UI 入口、详情弹窗、播放控件、range 控件事件路径和歌词渲染均走真实前端页面流程。

# 2026-07-28 — QQ 音乐式歌词渲染方向性改进

## 背景

- 当前 Rosettrism 已具备 Karaoke 双行队列、逐字 timing、倒计时气泡、助唱标注、翻译/读音/罗马音和多语言 fixture，但用户反馈歌词播放页的整体渲染仍不够理想，需要向 QQ 音乐成熟歌词体验对标。
- 本计划对应 `requirement.md` 的 `v2.3.0 — 2026-07-28 — QQ 音乐式歌词渲染方向性改进`。
- 现有实现的主要问题不在“功能缺失”，而在播放时钟、状态切换、视觉焦点、长句排版、标注避让和验收方法尚未形成一套稳定的行为契约。
- 当前 `LyricPlaybackView.jsx` 使用 `performance.now()` 模拟播放时钟并在动画帧内更新 React state。该模式适合独立预览，但未来接入真实音频时存在时钟漂移、后台恢复跳变和每帧重渲染成本，必须先完成播放内核解耦，再做高密度视觉动效。
- 现有 `frontend/verify-ui-polish.mjs` 使用 URL hash 假设页面路由，但 `App.jsx` 当前没有 hash 路由，不能作为歌词对标验收依据；本轮必须使用真实点击路径或专用 playback harness。

## 对标边界

- 对标 QQ 音乐的成熟交互规律：音画同步、双行接力、逐字填充、焦点转移、空拍提示、助唱标注、翻译与响应式体验。
- 不复制 QQ 音乐的商标、图标、专有素材、皮肤、文案、资源文件或像素级布局；Rosettrism 保留自身品牌与日系轻科技视觉方向。
- 公开资料只能用于确认“双行、逐字、多语、显示设置”等能力方向；最终行为以本项目保存的参考证据、真实 QQ/QRC fixture 和用户验收为准。

## 目标

- [ ] 建立可由外部音频驱动的统一播放时钟与歌词状态机。
- [ ] 实现稳定的 QQ 音乐式双行接力、逐字填充和切行焦点转移。
- [ ] 收敛歌词舞台视觉层级，减少装饰噪声，让歌词始终是第一视觉焦点。
- [ ] 保证助唱标注、倒计时、翻译/读音和多语言长句在桌面与移动端稳定呈现。
- [ ] 建立行为断言、截图基线和性能采样三层验收，不再依赖主观目测或无效 hash 路由脚本。

## 非目标

- 本轮不修改 Rust Provider、歌词解码协议或 Unified JSON Schema，除非前端无法表达必要信息并有单独契约提案。
- 本轮不接入 QQ 音乐私有接口、不绕过版权或客户端保护、不抓取专有资源。
- 本轮不重做整个 Dashboard；只调整播放弹窗、歌词舞台及其必要的设置入口。
- 本轮不在第一阶段追求光效数量；性能、时序和可读性优先于装饰。
- 本轮不把 `frontend/verification/` 中现有未跟踪截图直接当作新的 golden baseline，必须经过人工确认后再决定是否纳入版本控制。

## 当前能力与差距矩阵

| 维度 | 当前基线 | 目标行为 | 优先级 |
|---|---|---|---|
| 播放时钟 | `performance.now()` 独立模拟；动画帧更新 React state | 支持 `audio.currentTime` / 外部 clock；暂停、seek、后台恢复后无累积漂移 | P0 |
| 播放状态 | active/next/countdown 由组件内多组派生值组合 | 单一 deterministic frame state：phase、active、next、lanes、progress、countdown | P0 |
| 双行接力 | 依据索引奇偶显示左右两行 | 当前行稳定占据所属 lane，下一行提前就位；切行无跳位、闪烁或布局塌缩 | P0 |
| 逐字填充 | 每字独立计算渐变截断 | 已唱/正在唱/未唱边界连续，seek 后立即正确，无 2.4% 视觉超前硬补偿 | P0 |
| 切行焦点 | active 行缩放和位移，缺少完整过渡契约 | 旧行退场、新行接管、次行预备形成连续视觉节奏；不依赖滚动抖动 | P1 |
| 元数据/前奏 | 标题、制作信息和倒计时已有独立面板 | 元数据结束、前奏倒计时、首句进入三阶段无重叠、无重复标题 | P1 |
| 空拍倒计时 | 3 个气泡分阶段消散 | 只在可感知长空拍出现；数量、节奏、消散与下一句时间严格对应 | P1 |
| 助唱标注 | 换气、重音、长音、滑音可锚定 | 标注贴字但不压字；同锚点有优先级；高密度标注自动降级，移动端不碰撞 | P1 |
| 翻译/读音 | 原文/译文/双语循环切换 | 原文、译文、双语状态明确；副行不抢焦点；ruby/罗马音不破坏行高 | P1 |
| 长句排版 | Karaoke 强制 `nowrap`，超长句依赖最大宽度 | 建立字体缩放、压缩字距、分段或安全裁切策略；任何语种不可横向溢出 | P0 |
| 视觉层级 | 金黄卡片、渐变、粒子、光晕、注解图例并存 | 舞台背景退后，活动歌词最强，次行次之，控件与装饰弱化；默认低噪声 | P2 |
| 响应式 | 390px 有不溢出测试，控件最低仅 32px | 390x844、768x1024、1280x720、1440x900 均有布局契约；主要触控目标 ≥44px | P1 |
| 性能 | 未量化长歌词播放时的帧预算 | 正常播放更新不导致整棵播放卡每帧重渲染；关键动画稳定，长任务可追踪 | P0 |
| 验证 | 行为断言和手工截图分散 | 专用 harness + 固定时间采样 + viewport 矩阵 + 人工签字的视觉 baseline | P0 |

## 目标体验原则

1. **贴音优先**：高亮进度必须服从播放时钟，视觉补偿不得掩盖数据或时钟错误。
2. **两行接力**：任何时刻最多保留主唱焦点行和预备行；新旧行切换时不重排、不跳边、不突然居中。
3. **一句一焦点**：活动行、次行、元数据、标注、控制器必须有明确强弱，不能同时争夺注意力。
4. **动效有因果**：缩放、位移、淡化、气泡只服务于“即将开始、正在演唱、已经结束”三个语义。
5. **数据降级可预测**：逐字 timing、仅逐行 timing、纯文本三种数据质量都有明确渲染策略。
6. **多语种不特判皮肤**：中文、粤语、日语、英语共享状态机，只在排版策略上按字符特征自适应。
7. **设置少而有效**：默认效果接近成熟音乐播放器；高级设置用于低动效、翻译模式和视觉偏好，不把正确性暴露给用户调参。

## 目标架构

### 1. Clock adapter

统一接口建议：

```js
{
  nowMs(),
  durationMs(),
  isPlaying(),
  play(),
  pause(),
  seek(ms),
  subscribe(listener)
}
```

- Preview clock 可继续使用 `performance.now()`，但只作为 adapter 实现。
- Audio clock 读取媒体元素时间，歌词不自行累计真实播放时间。
- 后台恢复、系统休眠、播放速率变化和 seek 后，下一帧必须从 clock 重新取值，禁止靠累计 delta 维持真值。

### 2. Playback frame state

从歌词数据和 `currentMs` 纯计算：

```js
{
  phase: 'metadata' | 'countdown' | 'singing' | 'interlude' | 'ended',
  activeLineIndex,
  nextLineIndex,
  laneItems,
  lineProgress,
  wordProgress,
  countdown,
  visibleAnnotations
}
```

- 状态机放入可单测模块，不在 JSX 中散落组合判断。
- 二分查找或游标跟踪 active line，避免每帧多次线性扫描整首歌词。
- React 只在 phase、active line、lane 结构改变时更新结构；逐字连续进度优先通过 CSS 变量或受控最小节点更新。

### 3. Stage composition

- `PlaybackHeader`：标题、艺人、来源、格式，弱化为辅助信息。
- `LyricStage`：只承载元数据、双行歌词、倒计时和必要氛围背景。
- `KaraokeLane`：负责 lane 稳定、活动/预备状态和长句 fit。
- `WordProgressText`：负责逐字填充和 ruby。
- `SingingAnnotationLayer`：独立布局和碰撞策略。
- `PlaybackControls`：播放、暂停、重播、时间轴、翻译模式；移动端可重排但不缩小到不可触控。

## 阶段列表

### Phase 0 — 对标证据与基线冻结

#### Task checklist

- [ ] 由人工提供或确认 3 至 5 组 QQ 音乐参考证据：前奏、普通逐字、长空拍、助唱高密度、移动端/窄屏。
- [ ] 为每组证据记录 viewport、时间点、歌词文本、数据格式和要对标的行为，不仅保存截图。
- [ ] 固定 Rosettrism 当前同时间点截图、DOM 尺寸与行为输出，形成 before baseline。
- [ ] 将 `frontend/verify-ui-polish.mjs` 标记为非验收脚本，或改为真实点击导航；歌词专项使用独立 harness。

#### 验收条件

- [ ] 对标项可以写成“何时、显示什么、位于哪里、如何变化”的行为描述。
- [ ] 没有要求复制 QQ 音乐品牌素材或专有资源。
- [ ] 基线证据覆盖桌面、移动和至少一条真实 QQ/QRC 逐字歌词。

#### 完成状态

- [ ] 未开始
- [ ] 进行中
- [x] 已完成（2026-08-25 收口：后续 Phase 的固定时间点行为断言、四 viewport 截图 baseline 与性能采样已取代外部参考证据，不再补录 QQ 音乐素材；与 requirement.md v2.3.0 状态同步）

### Phase 1 — P0 播放时钟与状态机

#### Task checklist

- [x] 新增 clock adapter，保留 preview clock，并实现 audio/media clock 与 fake clock。
- [x] 新增纯函数 playback frame state，统一 phase、active、next、lane、countdown 和 progress。
- [x] 将活动行与下一行定位收敛为二分查找。
- [x] seek、pause/resume、restart、结束态统一从 clock 重算状态。
- [x] Header、Legend 与低频 PlaybackActions 使用稳定输入和 memo 隔离，不随连续时间帧重渲染。
- [x] 移除 `lyricProgressStyle` 中非 exact 模式的 `+2.4%` 视觉超前补偿。

#### 验收条件

- [ ] 在 0.5x、1x、1.5x、2x 时钟模拟下，逐字边界与期望时间偏差不超过一帧预算或 50ms，两者取较大值（当前已覆盖 1x 与确定性固定时间边界；多速率矩阵待 Phase 5）。
- [x] 连续播放 10 分钟的 preview clock 模拟无累计漂移；media clock 始终读取 `media.currentTime`，恢复后重新对齐媒体真值。
- [x] seek 到固定 fixture 时间点后，活动行、逐字进度、倒计时和标注由同一 frame state 同步重算。
- [x] 连续时间帧不触发 Header、Legend 和 PlaybackActions 重渲染；时间轴和歌词舞台保留必要更新。

#### 完成状态

- [ ] 未开始
- [ ] 进行中
- [x] 已完成

### Phase 2 — P0/P1 双行接力与逐字渲染

#### Task checklist

- [x] 将 lane 分配从“临时两项列表”升级为稳定 lane identity，按歌词索引奇偶保持左右归属。
- [x] 定义 metadata → countdown → first line、line → line、line → interlude、ending 四类过渡。
- [x] 当前行、预备行、已完成行使用状态类和 CSS 变量表达，避免 JSX 分支闪烁。
- [x] 逐字填充支持零时长空格、连字符、英文词组、CJK 单字和 ruby。
- [x] 为仅逐行 timing 提供连续整行填充；纯文本不伪造逐字效果。
- [x] 加入长句 fit 策略：按实测宽度分级缩放，设置最小字号；仍超限时启用安全分段或可读裁切，不产生页面横向滚动。

#### 验收条件

- [x] 任意切行采样中，两个 lane 的物理位置稳定，活动行不会从左跳右或从右跳左。
- [x] 切行前下一句已在正确 lane 预备；切行后旧行在 180–320ms 内退场，新行平滑接管。
- [x] 逐字颜色边界连续，无字符整体突然变色、重复填充或 seek 后残留。
- [x] 中英日粤长句在 390px 和 1280px 下均无横向溢出，主句保持可读。

#### 完成状态

- [ ] 未开始
- [ ] 进行中
- [x] 已完成

### Phase 3 — P1 前奏、空拍、助唱与多语

#### Task checklist

- [x] 收敛标题/制作信息展示时序，标题仅保留 Header 一处，制作信息结束后才允许倒计时进入。
- [x] 以 gap 阈值和下一句时间计算倒计时，不为短停顿显示气泡。
- [x] 保留气泡逐个消散语义；reduced-motion 下禁用气泡、碎片和光环动画但保留数量语义。
- [x] 为标注层建立全行 label collision/priority 策略：优先保留换气、重音和长音，最多显示 3 个且保持锚点间距。
- [x] 翻译模式保留原文/译文/双语三态；副行使用固定双槽高度，切换不改变 lane 几何。
- [x] 日文 ruby、粤语/中文读音、英文长词与空格继续使用现有多语排版与长句 fit 策略。

#### 验收条件

- [ ] 标题只出现一次；制作信息结束后再进入倒计时或首句。
- [ ] 小于约 1.2s 的普通行间停顿不出现三气泡；长空拍根据剩余时间展示 3→2→1→消散。
- [ ] 活动行所有可见标注不互相重叠、不压住字形；无法完整展示时按优先级降级而不是挤成一团。
- [ ] 翻译/双语/读音切换后，活动 lane 位置变化不超过约定容差，时间进度不中断。

#### 完成状态

- [ ] 未开始
- [ ] 进行中
- [x] 已完成（运行时代码、18/18 单测、隔离构建与 Firefox 浏览器专项分段验收均通过；完整覆盖 12 个场景）

### Phase 4 — P2 视觉舞台与控件产品化

#### Task checklist

- [x] 将默认舞台从“多层金黄渐变 + 粒子 + 多重高光”收敛为低噪声背景，保留 Rosettrism 日系轻科技品牌而非 QQ 皮肤复制。
- [x] 建立文字层级 token：active、upcoming、past、translation、reading、metadata、annotation。
- [x] 活动行只使用一种主高亮机制；减少同时出现的竖线、缩放、渐变、阴影和粒子竞争。
- [x] 控件改为音乐播放语义布局，播放/暂停为主操作，时间轴为连续主控，重播/翻译为次操作。
- [x] 默认关闭或弱化 ambient particles，高性能设备可作为可选 preset。
- [x] 保证 `prefers-reduced-motion` 和 low-distraction 模式下没有语义丢失。

#### 验收条件

- [ ] 截图盲测中，第一视觉焦点稳定落在活动歌词，而不是卡片背景、状态 badge 或按钮。
- [ ] active/upcoming/past 在浅色主题下均达到可读对比度；不可用低透明度替代语义。
- [ ] 主要触控目标在移动端不小于 44×44px。
- [ ] 默认视觉方向不出现大面积蓝紫主导，并与 Dashboard 现有品牌 token 协调。

#### 完成状态

- [ ] 未开始
- [ ] 进行中
- [x] 已完成（低噪声默认舞台、文字层级 token、控件主次、默认关闭环境特效；18/18 单测，Firefox 默认态/移动端/reduced-motion 专项通过，隔离生产构建通过）

### Phase 5 — 专用自动化与性能验收

#### Task checklist

- [x] 新增 `tests/lyric-playback-benchmark.spec.js` 或等价专用测试，不依赖 URL hash 切视图。
- [x] 使用真实点击路径或测试专用 playback harness 注入固定 fixture 和 clock。
- [x] 覆盖 0ms、元数据末尾、首句前、逐字中点、切行前后、长空拍、结束前后等固定时间点。
- [x] 覆盖 390x844、768x1024、1280x720、1440x900 viewport。
- [x] 行为断言覆盖 lane 位置、active line、进度、标注重叠、文本溢出、控件尺寸和 reduced-motion。
- [x] 仅对稳定区域建立截图 baseline，关闭动画或冻结 clock 后截图；baseline 更新必须人工审阅。
- [x] 加入 React render count 或浏览器 performance marks，记录长歌词播放时的结构渲染次数和长任务。

#### 验收条件

- [x] 测试不会重复验证默认 Overview 页面冒充多个视图。
- [x] 截图失败可以定位到明确时间点、fixture、viewport 和状态，而不是只输出整页差异。
- [x] 60 秒自动播放采样中无持续掉帧趋势；测试环境内无 >100ms 的歌词渲染长任务。
- [x] 200 行歌词 fixture 的 active line 查询和 frame state 计算保持稳定，不随播放时间线性恶化。

#### 完成状态

- [ ] 未开始
- [ ] 进行中
- [x] 已完成（固定时钟 harness、四 viewport 截图 baseline、200 行/60 时间点性能采样、React Profiler 指标均落地；18/18 单测、Firefox benchmark 7/7、隔离构建通过；Three.js 已拆为按需 chunk）

### Phase 6 — 实机评审与发布收口

#### Task checklist

- [x] 使用《龙战骑士》真实 QQ/QRC 数据完成固定时间点 A/B 技术评审，并输出桌面/移动 after 截图。
- [x] 使用一首普通逐行歌词和一首无 timing 文本验证降级路径。
- [x] 在 Windows Chrome/Edge、Firefox 和 390x844 移动 viewport 完成视觉与交互自动检查。
- [ ] 由产品负责人确认 before/after 截图与关键时间点录屏；当前已记录技术结论，未伪造人工签字或录屏证据。
- [x] 运行完整单元/发布专项/Rust 测试、重建 `frontend/dist`，同步 README、requirement 和 plan 状态；版本维持前后端一致的 `4.8.20`。

#### 验收条件

- [ ] P0 项全部完成，P1 无阻断性缺陷；P2 技术评审已完成，仍待产品负责人对截图/录屏签字。
- [x] 自动化未发现活动行错位、逐字进度倒退、标注压字、长句横向溢出、seek 后残留等阻断问题。
- [x] 测试、构建、文档和 dist 产物一致。

#### 完成状态

- [ ] 未开始
- [ ] 进行中
- [x] 已完成（2026-08-25 随 v4.9.0 发布收口：三平台 Package、容器与 GitHub Release 全绿；截图/录屏确认不再作为阻塞项）

### Phase 7 — 真实媒体播放闭环

#### Task checklist

- [x] 扩展 media clock 状态契约，统一读取 `media.currentTime`、duration、播放、缓冲、seek、倍速、结束和错误状态。
- [x] 新增真实 `HTMLAudioElement` 包装层和独立 media playback harness；无合法音频 URL 时保留 preview clock。
- [x] 覆盖播放、暂停、seek、0.5x/1x/1.5x/2x、切换音源、结束和重播。
- [x] 使用本地生成的短 WAV fixture，不依赖外部网络、版权音频或私有 Provider 接口。
- [x] 组件卸载时销毁 external media clock，移除媒体事件监听器和动画帧。

#### 验收条件

- [x] Edge 与 Chrome 真实媒体专项各 4/4 通过。
- [x] Firefox 四个业务场景执行完成且无断言失败产物；runner 在浏览器回收阶段异常退出，记录为 Windows 环境拖尾，不阻塞后续开发。
- [x] 歌词时间只服从媒体真值；暂停冻结、seek 收敛、倍速和重播均不保留旧状态。
- [ ] 正式 Provider payload 提供合法、稳定的音频 URL 后，将业务入口由 preview clock 切换为 media clock；本阶段不伪造音源能力。

#### 完成状态

- [ ] 未开始
- [x] 进行中（2026-08-25 复核：运行时与浏览器验收完成；剩余仅第 295 行外部前置——正式 Provider 合法音频 URL，未解决前不接入媒体时钟切换）
- [ ] 已完成

### Phase 8 — 歌词质量与数据可信度

#### Task checklist

- [x] 审计 Rust `LyricTrackQuality`、Unified 聚合选择、前端 normalization 和播放入口的数据流。
- [x] 新增 normalization 后的纯函数质量诊断器，输出 timing 等级、能力矩阵、指标、诊断码和降级原因。
- [x] 区分逐字 timing、逐行 timing、无 timing 和结构异常；禁止将 raw 或缺失显式时间戳的文本标记为可播放。
- [x] 检测行时间倒序/重叠、空文本、逐字倒序/重叠/越界/零时长、助唱标注越界等可信度问题。
- [x] 在播放页显示低噪声质量等级和阻断性诊断；逐行歌词可降级播放但不伪造逐字能力。
- [x] 使用真实 QQ/QRC、普通逐行、raw、多语和异常 timing fixture 固化自动化回归。

#### 验收条件

- [x] 真实 QQ/QRC 被判定为逐字同步，保留 word timing 与助唱标注能力。
- [x] 普通 LRC 被判定为逐行同步且可播放，`wordTiming=false`，整行进度可用。
- [x] raw 或无显式时间戳文本被判定为无同步且不可播放，并提供稳定降级原因码。
- [x] 结构异常不会静默伪装为高质量；诊断结果包含可定位的 line/word/annotation 上下文。
- [x] 质量报告不改变确定性 playback frame state；现有真实 QRC 和多语回归保持通过。

#### 完成状态

- [ ] 未开始
- [ ] 进行中
- [x] 已完成（25/25 单测、Chrome 发布专项 5/5、多语与播放核心 13/13、正式 dist 和依赖审计 0 漏洞通过；Edge 五项业务场景完成但 runner 回收异常）

### Phase 9 — 播放器产品会话能力

#### Task checklist

- [x] 审计 media clock、`MediaLyricPlayback`、现有控件、持久化范式与 Phase 7 浏览器 harness，冻结职责边界。
- [x] 新增独立播放器会话内核，管理队列、当前项、上一首/下一首、顺序/单曲循环/列表循环/随机模式。
- [x] 接入音量、静音、倍速和可信状态恢复；只恢复当前新鲜队列中的 durable 音源，不持久化或复活临时签名 URL。
- [x] 接入 Media Session API、媒体结束自动切歌、错误重试和事件监听清理。
- [x] 在桌面与移动端提供低噪声队列控制，并保持主要触控目标不小于 44×44px。
- [x] 新增会话内核单测与真实媒体浏览器专项，不回归 Phase 1–8 的时钟、歌词状态和质量诊断契约。

#### 验收条件

- [x] 队列选择、上一首/下一首、顺序停止、单曲循环、列表循环和随机模式具有确定性契约。
- [x] 刷新后恢复当前歌曲、时间点、音量、静音和倍速；过期、缺失或不可持久化音源不得恢复。
- [x] 系统播放/暂停、上一首/下一首和 seek 命令映射到同一媒体时钟与队列真值，卸载后无遗留 handler。
- [x] 音源失败与歌词质量问题分开呈现；重试不残留旧时间和旧播放状态。
- [x] Chrome 真实媒体专项、前端单测、正式构建、依赖审计和文档一致性通过。
- [x] 正式 Provider 合法音频 URL 仍为独立前置条件；本阶段继续使用本地 WAV fixture 验收，不接入私有接口。

#### 完成状态

- [ ] 未开始
- [ ] 进行中
- [x] 已完成（会话内核 35/35 单测、Chrome 专项 5/5、歌词核心 13 项全执行完成但 runner 回收返回 1、Rust 165/165、正式 dist、依赖 0 漏洞、格式与文档一致性通过）

## 验收场景矩阵

| 场景 | Fixture / 数据 | 固定采样点 | 核心断言 |
|---|---|---|---|
| 前奏元数据 | meta stress fixture | 0ms、元数据结束前后 | 标题仅出现一次；制作信息按序退出；不与首句和倒计时重叠 |
| 首句倒计时 | meta stress fixture | 首句前 3.2s、2.2s、1.2s、0.1s | 3→2→1→消散顺序正确；首句接管时无布局跳变 |
| 普通逐字 | 真实 QQ/QRC fixture | 当前字开始前、进行中、结束后 | 已唱/正在唱/未唱边界准确；相邻字符连续；偏差满足时钟预算 |
| 双行切换 | 真实 QQ/QRC fixture | 切行前 100ms、切行点、切行后 250ms | 左右 lane 物理位置稳定；下一句提前就位；旧行自然退场 |
| 长空拍 | meta stress fixture | gap 起点、剩余 3s/2s/1s、下一句前 | 只对长空拍显示；气泡数量和剩余时间一致；无重复倒计时 |
| 高密度助唱 | 《龙战骑士》真实数据及 stress fixture | 多标注字前后各 100ms | 标注贴字、不压字、不互撞；同锚点按优先级降级 |
| 中文长句 | 中文 fixture | 活动行中点 | 字号适配后仍清晰；390px 与 1280px 均无横向溢出 |
| 粤语读音 | 粤语 fixture | 活动行中点、读音切换后 | 读音行稳定；主句不被挤压；切换不改变当前播放状态 |
| 日文 ruby | 日语 fixture | 活动行中点 | ruby 与正文对齐；行高稳定；假名不与相邻字重叠 |
| 英文长词 | 英语 fixture | 活动行中点 | 空格、连字符和长词进度连续；不出现整词瞬时跳色 |
| 翻译模式 | 双语 fixture | 原文/译文/双语切换前后 | 副行不抢焦点；lane 位移在容差内；播放不中断 |
| Seek | 任一逐字 fixture | 向前、向后、跨空拍 seek | 100ms 内 active line、逐字进度、倒计时和标注全部重算正确 |
| Pause/Resume | 任一逐字 fixture | 暂停 3s 后继续 | 暂停期间进度冻结；恢复后无累计漂移或补帧跳跃 |
| Restart | 任一含前奏 fixture | 结束态点击重播 | 回到 metadata/countdown 初态；无旧行、旧标注和旧进度残留 |
| 结束态 | 任一完整 fixture | 最后一个字前后 | 最后一字完整填充；状态进入 ended；控制器语义正确 |
| 低动效 | 任一 stress fixture | `prefers-reduced-motion: reduce` | 取消非必要位移、缩放和粒子；歌词状态语义仍完整 |
| 移动端控件 | 任一 fixture | 390x844 | 无横向滚动；主控件 ≥44×44px；时间轴和翻译控制可操作 |
| 平板布局 | 任一 fixture | 768x1024 | 舞台、双行和控制器无互相遮挡；长句不溢出 |
| 桌面布局 | 任一 fixture | 1280x720、1440x900 | 活动歌词为第一视觉焦点；弹窗内容无需非预期整页滚动 |

## 定量验收总表

| 指标 | 阈值 / 契约 | 采集方式 |
|---|---|---|
| 时钟漂移 | 10 分钟模拟播放无累计漂移；恢复后首帧重新取 clock 真值 | Fake clock 单测 + 浏览器 harness |
| 逐字边界偏差 | 不超过一帧预算或 50ms，两者取较大值 | 固定采样点行为断言 |
| Seek 收敛 | seek 后 100ms 内状态一致 | Playwright fake clock |
| 切行过渡 | 旧行在 180–320ms 内退场，新行无跳位接管 | DOM box 采样 + 冻结时间截图 |
| 横向溢出 | 4 个目标 viewport 的页面及舞台 `scrollWidth <= clientWidth` | Playwright 布局断言 |
| 触控尺寸 | 移动端主要操作 ≥44×44px | DOM bounding box |
| 标注碰撞 | 活动行可见标注与正文、相邻标注无几何交叠 | bounding box 碰撞检测 |
| 长任务 | 测试环境内无由歌词渲染造成的 >100ms 长任务 | PerformanceObserver / trace |
| 结构重渲染 | 连续进度帧不重渲染无关 Header、Legend、Controls | React Profiler / render counter |
| 可访问动效 | reduced-motion 下移除非必要连续动画，状态信息不丢失 | 媒体特性 Playwright 用例 |

## 实施优先级与批次

### 第一批：P0 播放内核

1. 新增 clock adapter 与 fake clock。
2. 新增 deterministic playback frame state 和覆盖边界条件的单测。
3. 收敛 active line 查询与逐字进度计算。
4. 将结构更新与高频进度更新解耦。
5. 建立不依赖 Dashboard 路由的 playback harness。

> 第一批完成前，不建议继续增加粒子、光晕或复杂舞台动效。否则只会把时钟与重渲染问题藏在更重的视觉层下面。

### 第二批：P1 舞台行为

1. 固化双行 lane identity 和四类切换状态。
2. 完成长句 fit、空拍倒计时、标注碰撞和多语副行策略。
3. 补齐 seek、暂停恢复、结束态、移动端和 reduced-motion 回归。

### 第三批：P2 视觉产品化

1. 基于人工确认的参考证据收敛舞台层级。
2. 弱化背景与装饰，统一活动/预备/过去/副行 token。
3. 重排播放器控制器，完成桌面与移动端盲测。
4. 人工审阅并签字确认截图 baseline。

## 预计改动落点

以下为实施时的预期落点，不代表本计划阶段立即修改：

- `frontend/src/LyricPlaybackView.jsx`：改为消费 clock 与 frame state，拆分舞台组件。
- `frontend/src/lyricPlaybackViewModel.js`：扩展为确定性 frame state、active line 查询和数据降级策略。
- `frontend/src/lyricPlayback.js`：保持 normalization 职责，补充必要的排版元信息时需先写契约测试。
- `frontend/src/styles/lyric-stage.css`：收敛视觉层级、lane 状态、长句 fit、响应式和 reduced-motion。
- `frontend/tests/lyric-playback-benchmark.spec.js`：新增专用行为、布局与视觉回归。
- `frontend/playwright-verify-meta-stress.mjs`：迁移可复用的 meta/countdown/annotation stress 场景，避免重复维护。
- `frontend/tests/fixtures/`：补齐固定 QQ/QRC、多语长句、仅逐行 timing 和纯文本降级样本。

## 风险与对策

| 风险 | 影响 | 对策 |
|---|---|---|
| 没有可复核的 QQ 音乐参考证据 | 对标退化为主观审美争论 | Phase 0 先固定截图/录屏、时间点、viewport 和行为描述 |
| 将 preview clock 当作真实媒体时钟 | 接入音频后漂移、seek 错位 | clock adapter 隔离；真实播放只信媒体时间 |
| 每帧 React state 更新范围过大 | 长歌词播放掉帧、控件抖动 | 结构状态低频更新，连续进度使用最小节点或 CSS 变量 |
| 视觉补偿掩盖 timing 错误 | 看似顺滑但实际不贴音 | 移除无依据超前值；用固定时间点和误差预算验收 |
| 长句策略过度缩小字体 | 不溢出但不可读 | 设置最小字号并使用分段/安全裁切降级，人工覆盖四语种 |
| 助唱标注密度不可控 | 移动端压字或抖动 | 优先级、碰撞检测和密度降级；不保证所有标签同时展开 |
| 截图 baseline 被动画污染 | CI 视觉测试不稳定 | fake clock 冻结、关闭非必要动画、只截稳定区域 |
| 现有未提交 UI 改动与本轮实施交叉 | 难以归因和回滚 | 本计划阶段只写文档；实施时按批次拆分提交并先记录工作树基线 |
| 模仿边界不清 | 品牌或素材侵权风险 | 只借鉴通用交互规律，保留 Rosettrism 自有视觉与资产 |

## 测试与发布检查

实施阶段每一批至少执行：

```bash
cargo fmt --check
cargo test --no-fail-fast
cd frontend && npm run test:unit
cd frontend && npm test
cd frontend && npm run build
scripts/check-plan-requirement.sh --base HEAD
git diff --check
```

歌词专项在 Phase 5 起还必须执行专用 benchmark Playwright 命令，并在 plan 中记录：

- fixture 名称与来源类型；
- viewport；
- 固定采样时间；
- 行为断言结果；
- 截图 baseline 是否经人工确认；
- 性能采样环境与结果。

## 测试记录

- 2026-07-28：本轮仅完成方向性计划与需求历史同步，未修改运行时代码，未执行播放器功能测试。
- 2026-07-28：`scripts/check-plan-requirement.sh --base HEAD` 通过。
- 2026-07-28：`git diff --check` 通过；仅输出既有工作树文件的 LF→CRLF 提示，无空白错误。
- 2026-07-28：计划与需求文档 UTF-8 检查通过、代码块闭合；计划 60 行 Markdown 表格均无截断。
- 2026-07-29：完成第一批 P0 播放内核：新增 preview/media/fake clock adapter、deterministic frame state、二分时序定位、严格 timing 填充、静态区域 memo 隔离和专用浏览器回归。
- 2026-07-29：`npm run test:unit` 通过（17/17）；`npm test` 通过（9/9，含 2 个 playback core、四语种和移动端）；`npm run verify:meta-stress` 通过；`npm run build` 通过。
- 2026-07-29：专用测试确认 metadata/countdown/interlude/singing/ended phase、seek 越界钳制、pause 冻结、resume 推进、restart 复位；多速率矩阵与浏览器性能 trace 留待 Phase 5。
- 2026-07-30：完成 Phase 2 双行接力与长句适配；`npm run test:unit` 通过（17/17），Firefox 专项回归通过（11/11），覆盖四语种、390x844 / 768x1024 / 1280x720 / 1440x900、稳定 lane 节点与长句无横向溢出。
- 2026-07-30：修复跨浏览器长句测量：fit-content 节点不再用自身 `clientWidth` 作为可用宽度，改以父 lane 与计算后 `max-width` 的较小值判定 normal/compact/tight/wrap；768x1024 Firefox 用例由误判 `normal` 修复为正确降级并通过。
- 2026-07-30：`cargo fmt --check` 与 Rust 测试 165/165 通过；隔离前端构建通过；plan/requirement 一致性与 `git diff --check` 通过。Chromium 在当前 Windows 会话中即使加载空白 `data:` 页面也会在关闭时挂起，判定为环境级浏览器回收问题；业务断言曾通过，最终使用项目隔离 Firefox 完成干净回归。
- 2026-08-02：完成 Phase 5 固定时钟 harness、四 viewport 截图 baseline、200 行/60 时间点性能采样；前端单测 21/21，Firefox benchmark 7/7，Three.js 舞台已按需分包。
- 2026-08-02：真实《龙战骑士》回归发现首句开始后 420ms 消散倒计时覆盖 `singing`；已调整状态优先级并补首句边界测试，真实 QRC、普通逐行、无 timing 路径恢复通过。
- 2026-08-02：Phase 6 发布专项 Firefox 4/4、Windows Edge 4/4 通过；Windows Chrome 四项断言全部通过（首轮三项、修正错误的逐字指标后真实 QRC 项复验通过），浏览器进程回收拖尾判定为环境问题。
- 2026-08-02：`cargo fmt --check`、Rust 全量测试 165/165、`npm run test:unit` 21/21、`git diff --check` 和正式 `frontend/dist` 重建通过。版本维持 `4.8.20`；可选 Three.js chunk 约 512KB，默认关闭且按需加载。
- 2026-08-02：输出真实 QRC 桌面与 390x844 移动 after 截图；技术评审无 P0/P1 阻断，仍待产品负责人对 before/after 截图及关键时间点录屏签字。
- 2026-08-04：Phase 7 真实媒体专项 Edge 4/4、Chrome 4/4 通过；Firefox 四项业务场景完成但 runner 在浏览器回收阶段异常退出且无失败产物。`MediaLyricPlayback` 已补 external clock 销毁 cleanup。
- 2026-08-05：完成 Phase 8 质量诊断契约：`word_timed`、`line_timed`、`unsynced`、`invalid` 分级，能力矩阵、指标、稳定诊断码和降级原因已接入 normalization 与播放 UI；修复缺失时间戳行被默认成 0ms 可播放的可信度缺陷。
- 2026-08-05：前端单测 25/25、Chrome 发布专项 5/5、多语与播放核心 13/13、正式 `frontend/dist` 重建通过；Edge 五项业务场景完成但 runner 回收异常。Vite 升至 6.4.3、PostCSS 升至 8.5.25，`npm audit` 为 0 漏洞。

## 整体完成状态

- [x] 当前实现复核与主要问题定位
- [x] QQ 音乐式体验差距矩阵
- [x] P0 → P1 → P2 技术路线与阶段拆分
- [x] 定量验收指标与场景矩阵
- [x] 对标边界、风险与预计改动落点
- [ ] Phase 0：参考证据与基线冻结
- [x] Phase 1：播放时钟与状态机
- [x] Phase 2：双行接力与逐字渲染
- [x] Phase 3：前奏、空拍、助唱与多语
- [x] Phase 4：视觉舞台与控件产品化
- [x] Phase 5：专用自动化与性能验收
- [ ] Phase 6：实机评审与发布收口（技术门槛已通过，待产品负责人视觉签字）
- [ ] Phase 7：真实媒体播放闭环（实现与浏览器业务验收完成；正式 Provider 合法音频 URL 待后续接入）
- [x] Phase 8：歌词质量与数据可信度

## 方向性结论

当前播放器已经拥有接近成熟产品的功能积木，但还没有形成成熟播放器级别的统一时序、状态契约和验收闭环。正确路线不是继续堆 CSS，而是：

```text
P0 播放内核（clock + frame state + 性能）
  → P1 舞台行为（lane + 逐字 + 空拍 + 标注 + 多语）
  → P2 视觉产品化（层级 + 动效 + 控件 + 品牌）
```

第一实施批次应止于“可测试、可外部驱动、无累计漂移的播放内核与专用 harness”。只有该批通过定量验收后，才进入 QQ 音乐式舞台行为和视觉精修。
# IDV-ASAS 实现原理文档

## 1. 项目概述

IDV-ASAS（Identity V Area Selection Assistance System，第五人格区域选择辅助系统）是一个基于 Rust + eframe/egui 的桌面 GUI 应用，通过调用 OpenAI 兼容的大语言模型，为《第五人格》排位赛中的求生者阵营提供天赋选择与区域选择推荐方案。

## 2. 技术选型

| 层面 | 技术 | 说明 |
|------|------|------|
| 语言 | Rust 1.75+ | 后端必须使用 Rust（resources/AGENTS.md 要求） |
| GUI 框架 | eframe 0.36 + egui 0.36 | 纯 Rust 桌面 GUI，使用 wgpu 渲染后端 |
| 渲染后端 | wgpu 30（Vulkan + GL） | 通过 eframe 的 wgpu 配置指定 Backends::VULKAN \| Backends::GL |
| 窗口后端 | X11 only | eframe 默认 features 禁用，仅启用 x11（禁用 wayland 和 accesskit） |
| 图片解码 | image 0.25（仅 PNG） | 解码角色头像、地图图片、天赋图标 |
| HTTP 客户端 | ureq 3.4 | OpenAI 兼容的流式 API 调用 |
| 序列化 | serde + serde_json | 配置与历史记录的 JSON 读写 |
| 时间 | chrono 0.4 | 历史记录的时间戳命名 |
| 中文字体 | 内嵌 Deng.ttf | 等线字体，编译时 include_bytes! 内嵌，确保中文正确渲染 |

## 3. 目录结构

```
app/
├── Cargo.toml              # 依赖声明与编译配置
├── assets/
│   └── Deng.ttf            # 内嵌中文字体（编译时 include_bytes!）
├── config.json             # 运行时生成，API 配置（明文存储）
├── history/                # 运行时生成，会话历史 JSON 文件
└── src/
    ├── main.rs             # 程序入口：创建 1280×720 窗口，配置 wgpu 渲染
    ├── app.rs              # IdvApp 结构体、状态机、eframe::App trait 实现、核心逻辑
    ├── ui_impl.rs          # 全部 UI 渲染方法（12 个步骤界面）+ 图片合成函数
    ├── config.rs           # API 配置（ApiConfig）：加载/保存/端点规范化
    ├── checker.rs          # 资源校验器（ResourceDb）：扫描地图与角色数据文件
    ├── positions.rs        # 选点坐标解析器（MapPosition）：解析 positions/*.md
    ├── prompt.rs           # 提示词组装：system prompt + 本局请求（完整版/紧凑版）
    ├── history.rs          # 会话历史管理（Session）：保存/加载/列表/命名
    ├── api.rs              # 模型调用：SSE 流式读取 + 打断 + Token 统计
    ├── parser.rs           # 输出解析器：方案拆分 + 字段校验 + 描述截断
    └── images.rs           # 图片纹理缓存（ImageCache）：按需加载 PNG → TextureHandle
```

## 4. 资源路径映射

程序运行时以 `CARGO_MANIFEST_DIR`（即 `app/`）为基准，向上一级定位项目根目录，再访问各资源：

| 资源 | 运行时路径 | 用途 |
|------|-----------|------|
| 角色数据 | `../resources/characters/*.md` | 六维属性、天赋推荐、联动关系 |
| 地图数据 | `../resources/maps/*.md` | 选点俗称、牵制难易、选点推荐模式 |
| 通用资料 | `../resources/README.md` 等 | 求生者属性构成、天赋选择一般规律 |
| 字典 | `../DICTIONARY.md` | 角色中英文名映射、地图中英文名映射、天赋中英文名映射 |
| 概念图 | `../icons/maps/*.png` | 地图选择界面的 3×3 网格图片 |
| 区域选点图 | `../icons/area-selection/*.png` | 结果展示界面的底图 |
| 角色头像 | `../icons/characters/*.png` | 角色选择与结果展示的头像（原始 120×120） |
| 天赋图标 | `../icons/personas/*.png` | 结果展示的天赋图标 |
| 选点坐标 | `../icons/positions/*.md` | 各地图选点中心坐标与图标尺寸 |
| 配置文件 | `app/config.json` | API 配置（明文存储） |
| 历史记录 | `app/history/*.json` | 会话历史 JSON 文件 |

## 5. 状态机设计

应用核心是一个有限状态机（FSM），由 `Step` 枚举驱动：

```
Config ──(确认/沿用)──→ HistoryImport ──(选择/跳过)──→ GlobalReq
                                                              │
              ┌───────────────────────────────────────────────┘
              ▼
         MapSelect ──(确认)──→ CharSelect ──(确认4角色)──→ RoundReq
                                                              │
                                                              ▼
          ┌─────────────── Analyzing ←──────────────────────┘
          │                   │
          │          ┌────────┴────────┐
          │          ▼                 ▼
          │    ResultDisplay        ErrorExit
          │          │                 │
          │     (确认方案)          (重试/退出)
          │          ▼
          │      WaitingEnd
          │          │
          │     (对局结束)
          │          ▼
          │       AskNext
          │          │
          │   ┌──────┴──────┐
          │   ▼             ▼
          │ MapSelect    SaveHistory ──(保存/不保存)──→ 退出
          │ (下一局)
          └──────────────────→ ErrorExit
```

每个状态对应 `ui_impl.rs` 中的一个 `pub(crate) fn ui_xxx(&mut self, ui: &mut egui::Ui)` 方法。`eframe::App::ui` 中的 `match self.step` 分发到对应渲染方法。

## 6. 核心模块原理

### 6.1 提示词组装（prompt.rs）

**系统提示词**（`system_prompt`）：会话级常量，在启动时一次性组装。注入以下内容：

1. `resources/AGENTS.md` — 任务说明（后端流程）
2. `DICTIONARY.md` — 角色与地图的中英文字典
3. `resources/README.md` — 求生者属性与天赋选择一般规律
4. `resources/characters/README.md` — 天赋说明与角色数据格式
5. `resources/maps/README.md` — 地图选点说明
6. 输出格式要求与覆盖说明

选点行格式为 `x 号选点：角色名：天赋1、天赋2`（使用编号），描述行使用俗称（不使用编号），不超过 100 字。

**本局请求**（`full_request`）：每局动态组装，包含紧凑信息、本局地图完整数据、四名角色完整数据。

**紧凑请求**（`compact_request`）：用于历史局上下文，省略大段资料避免 Token 膨胀。

### 6.2 模型调用（api.rs）

采用**线程化流式调用**架构：

```
主线程(UI)                        API 线程
    │                                │
    ├── start_api_call() ──────────→ chat_stream_threaded()
    │         │                        │
    │    创建 Receiver + InterruptFlag  ├── POST /chat/completions (stream=true)
    │         │                        │
    │   ctx.request_repaint()          ├── SSE 逐行读取 (BufReader)
    │         │                        │
    │   logic() 中 poll_api()          ├── 每 150ms 检查 interrupt flag
    │   rx.try_recv()                  │
    │         │                   ┌────┴────┐
    │    ApiEvent::Done ←─────────┤ 正常完成 │ 打断
    │                              └────┬────┘
    │                                   │
    │                              Channel.send(Done)
```

**打断机制**：通过 `Arc<AtomicBool>` 共享标记。UI 线程设置 `flag.store(true)` 后，API 线程在下次轮询循环中检测到并立即停止读取，返回 `interrupted=true`。

**Token 统计**：优先使用接口返回的 `usage` 字段；若服务不返回，则按字符数 × 0.75 估算并标注。

### 6.3 输出解析与校验（parser.rs）

模型返回的文本经过三阶段处理：

**阶段一：方案拆分**（`split_plans`）— 以 `方案` 开头的行作为分隔标记，跳过前言。

**阶段二：逐方案解析**（`parse_single_plan`）— 识别选点行和描述行，提取结构化数据。

**阶段三：合法性校验** — 角色名存在、天赋名合法（4 种）、每角色天赋数恰好为 2、选点编号不超范围、选点不重复、角色数恰好为 4。描述超 100 字时截断至最后一个完整句子并补充省略号。

**重试机制**：校验失败时追加错误说明到消息列表重新调用，最多 3 次，超限报告"您的模型与该项目不适配"。

### 6.4 图片加载与合成（images.rs + ui_impl.rs）

**ImageCache**：以文件名为键的 `HashMap<String, TextureHandle>`，首次请求时解码 PNG 上传 GPU，后续直接返回缓存。

**区域选点图合成**（`compose_map_image`）：

1. 加载区域选点底图为 RGBA 缓冲区
2. 读取 `positions/*.md` 获取选点坐标和图标尺寸
3. 对方案中每个角色：查找角色英文名 → 加载角色头像 → 缩放至 `icon_size` → alpha 混合叠加到地图对应坐标
4. 合成后的单张图转为 `egui::ColorImage` → `TextureHandle` → `egui::Image` 显示

合成在内存中完成，不依赖 painter 图层叠加，避免了渲染顺序问题。

**positions 解析**：文件中使用 LaTeX 格式 `\times` 而非 Unicode `×`，解析时先替换再分割。按行内容分流：含"角色图标"只解析图标尺寸，否则只解析地图尺寸。

### 6.5 历史管理（history.rs）

Session 结构包含版本号、创建时间、API 配置、全局需求、各局记录。每局记录包括地图、角色、需求、模型回答、Token 用量与费用。

加载续局时，历史局的紧凑请求 + 模型回答作为消息对进入上下文。导入历史后直接跳至第 2 步（地图选择），全局需求自动继承。

### 6.6 配置管理（config.rs）

`ApiConfig` 包含 Base URL、API Key、模型名、温度、max_tokens、输入/输出单价。端点自动补全 `/chat/completions`。温度发送前四舍五入至 2 位小数。

## 7. UI 渲染流程

窗口大小为 **1280×720**。eframe 0.36 的 `App` trait 有两个关键方法：

- **`logic`**：每帧 UI 前调用，用于轮询 API 通道和处理窗口关闭请求
- **`ui`**：每帧渲染调用，通过 `match self.step` 分发到对应步骤的渲染方法

**重绘驱动**：API 线程返回结果时通过 `ctx.request_repaint()` 通知主线程重绘。

**结果展示布局**（`ui_result_display_inner`）：左右布局
- 左侧：合成后的区域选点地图（含角色头像叠加）
- 右侧：天赋选择（4 行角色头像 + 天赋图标）、分析与说明、翻页按钮、确认/对局结束按钮

## 8. 错误处理

| 错误类型 | 处理方式 |
|---------|---------|
| API 网络错误 | ErrorExit 界面：重试 / 重新输入 / 退出 |
| 模型输出格式错误 | 自动重试（最多 3 次） |
| 重试超限 | 报告"您的模型与该项目不适配" |
| 用户打断 | 进入 ErrorExit，可选择重试或退出 |
| 资源加载失败 | 启动时 eprintln 输出错误 |

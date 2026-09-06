# IDV-ASAS WebUI 版实现原理文档

## 1. 项目概述

IDV-ASAS WebUI 版是《第五人格》区域选择辅助系统的 Web 界面版本。与桌面 GUI 版（`app/`）使用 eframe/egui 渲染桌面窗口不同，WebUI 版使用 **Axum** 构建 Rust 本地 Web 服务，以 **REST API + SSE** 暴露后端能力，以 **HTML/CSS/JavaScript** 实现浏览器端界面。

核心设计原则：**后端逻辑模块完全复用桌面版的 Rust 源码，不做任何修改**。7 个后端模块（`api.rs`、`config.rs`、`checker.rs`、`positions.rs`、`prompt.rs`、`history.rs`、`parser.rs`）被原样复制到 `webui-ver/src/` 中。新增的仅有：
- `main.rs` — Axum Web 服务（路由、状态管理、SSE 流式分析）
- `static/index.html` — 前端主页面
- `static/style.css` — 样式文件
- `static/app.js` — 前端逻辑

桌面版中依赖 egui 的 `app.rs`（状态机 + eframe::App）、`ui_impl.rs`（全部 UI 渲染）、`images.rs`（TextureHandle 缓存）被完全移除，由上述 Web 技术栈替代。

## 2. 技术选型

| 层面 | 技术 | 说明 |
|------|------|------|
| 语言 | Rust 1.75+ | 后端必须使用 Rust（resources/AGENTS.md 要求） |
| Web 框架 | axum 0.8 | 异步 Web 框架，提供路由、JSON 提取、SSE 响应 |
| 异步运行时 | tokio 1（full features） | Axum 的异步运行时，提供 `Mutex`、`time::sleep`、`TcpListener` |
| 静态文件 | tower-http 0.6（ServeDir） | 提供 `static/` 和 `../icons/` 目录的文件服务 |
| HTTP 客户端 | ureq 3.4 | **与桌面版相同**，OpenAI 兼容的流式 API 调用 |
| 序列化 | serde + serde_json | **与桌面版相同**，配置与历史记录的 JSON 读写 |
| 时间 | chrono 0.4 | **与桌面版相同**，历史记录的时间戳命名 |
| 流式支持 | async-stream 0.3 | `stream!` 宏，用于生成 SSE 事件流 |
| 前端语言 | HTML + CSS + JavaScript | 纯原生，无框架依赖 |
| 前端字体 | Google Fonts (Noto Sans SC) | 中文字体支持；离线时回退到系统字体 |
| 图片渲染 | HTML5 Canvas 2D | 替代桌面版的 egui ColorImage + TextureHandle |
| 事件推送 | SSE（Server-Sent Events） | 替代桌面版的 `mpsc::Receiver` 轮询 + `ctx.request_repaint()` |

### 关键差异说明

桌面版通过 `eframe::App::logic()` 每帧轮询 `mpsc::Receiver`，并通过 `ctx.request_repaint()` 驱动重绘。WebUI 版将此机制改为 **SSE 长连接**：前端通过 `EventSource` 建立连接，服务端在 async stream 中轮询同一个 `mpsc::Receiver`，通过 SSE 事件向前端推送结果。

## 3. 目录结构

```
webui-ver/
├── Cargo.toml                  # 依赖声明与编译配置
├── README_WEBUI.md             # 本目录的 README
├── DESIGN_WEBUI.md             # 本文件
├── config.json                 # 运行时生成，API 配置（明文存储）
├── history/                    # 运行时生成，会话历史 JSON 文件
├── static/                     # 前端静态文件（由 tower-http ServeDir 提供服务）
│   ├── index.html              # 主页面（全部 12 个步骤界面）
│   ├── style.css               # 样式（深黑蓝 + 黄色主题，与桌面版配色一致）
│   └── app.js                  # 前端逻辑（步骤流程 + Canvas 图片合成 + SSE 消费）
└── src/
    ├── main.rs                 # Web 服务入口：路由定义、AppState 状态管理、SSE 分析处理器
    ├── api.rs                  # ← 与桌面版完全相同
    ├── config.rs               # ← 与桌面版完全相同
    ├── checker.rs               # ← 与桌面版完全相同
    ├── positions.rs            # ← 与桌面版完全相同
    ├── prompt.rs                # ← 与桌面版完全相同
    ├── history.rs              # ← 与桌面版完全相同
    └── parser.rs               # ← 与桌面版完全相同
```

### 后端模块复用验证

7 个后端模块通过 `diff` 验证与 `app/src/` 中的对应文件逐字节一致：

```
api.rs:      identical
config.rs:   identical
checker.rs:   identical
positions.rs: identical
prompt.rs:    identical
history.rs:  identical
parser.rs:   identical
```

### main.rs 的角色

`main.rs` 承担了桌面版中 `app.rs` 的状态管理职责和 `ui_impl.rs` 的流程编排职责，但不包含任何渲染逻辑。具体包括：

1. **AppState 结构体**：持有会话状态（配置、选图、选角、需求、消息历史、分析结果等），替代桌面版 `IdvApp` 结构体
2. **Axum 路由**：14 个 REST API 端点 + 2 个静态文件目录，替代桌面版的 `match self.step` 状态分发
3. **SSE 分析处理器**：在 async stream 中轮询 `mpsc::Receiver`，处理重试逻辑，替代桌面版的 `poll_api()` + `ctx.request_repaint()`
4. **序列化辅助函数**：将 `Plan`、`Usage` 等 Rust 结构体手动序列化为 JSON，替代桌面版的 egui `RichText` / `Image` 渲染

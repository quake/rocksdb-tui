# RocksDB TUI 设计文档

## 概述

一个只读的 RocksDB 数据浏览器，通过 Secondary 模式安全连接本地数据库，支持声明式配置自定义数据解析。

## 技术选型

- **语言**: Rust
- **TUI 框架**: ratatui
- **RocksDB 绑定**: rust-rocksdb (支持 Secondary 模式)
- **配置格式**: TOML

## 架构分层

```
┌─────────────────────────────────┐
│           TUI Layer            │  <- 用户界面、键盘导航
├─────────────────────────────────┤
│         Parser Layer           │  <- JSON/MsgPack/Protobuf 解析
├─────────────────────────────────┤
│        Database Layer          │  <- Secondary 模式连接、迭代器
└─────────────────────────────────┘
```

## 命令行接口

```bash
rocksdb-tui --db /path/to/rocksdb --config ./parsers.toml

# 可选：指定 secondary 路径（默认自动生成到 /tmp）
rocksdb-tui --db /path/to/rocksdb --config ./parsers.toml --secondary /tmp/my-secondary
```

- `--db` 必填
- `--config` 可选，不传则所有 CF 用 hex 显示
- `--secondary` 可选，默认 `/tmp/rocksdb-tui-{hash}`

## 配置文件格式

配置文件仅定义 Column Family 的解析规则：

```toml
# parsers.toml

[[column_families]]
name = "users"
key_format = "string"
value_format = "json"

[[column_families]]
name = "orders"
key_format = "u64_be"
value_format = "protobuf"
proto_file = "./protos/order.proto"
proto_message = "Order"

[[column_families]]
name = "cache"
key_format = "string"
value_format = "msgpack"
```

## TUI 界面布局

```
┌─ Column Families ─┬─ Keys ─────────────┬─ Value ──────────────────┐
│                   │ [Search: ______ ]  │                          │
│ > default         │                    │ {                        │
│   users           │ user:1001          │   "name": "Alice",       │
│   orders          │ user:1002          │   "email": "a@test.com", │
│   cache           │ > user:1003        │   "created": 1706900000  │
│                   │ user:1004          │ }                        │
│                   │ user:1005  ▼ more  │                          │
├───────────────────┴────────────────────┴──────────────────────────┤
│ CF: users | Keys: ~1.2K (estimate) | Format: json | [?] Help      │
└───────────────────────────────────────────────────────────────────┘
```

### 三栏布局

- **左栏**: Column Family 列表，可上下切换
- **中栏**: 当前 CF 的 Key 列表，支持滚动和分页
- **右栏**: 选中 Key 的 Value，解析后格式化显示

### 分页行为

- 初始加载前 100 条 keys
- 滚动到底部自动加载下一批（无限滚动）
- 使用 RocksDB 迭代器的 seek 实现游标分页
- 显示 `▼ more` 提示还有更多数据

### 键盘操作

| 按键 | 功能 |
|------|------|
| `Tab` / `Shift+Tab` | 切换焦点栏 |
| `j/k` 或 `↑/↓` | 上下移动 |
| `gg` / `G` | 跳到顶部/底部 |
| `/` | 搜索（MVP: placeholder） |
| `q` | 退出 |
| `?` | 显示帮助 |

### 搜索功能（最终目标）

- `/`: 激活搜索框
- 精确匹配：输入完整 key，跳转到对应条目
- 前缀搜索：输入前缀，过滤显示匹配的 keys
- `Esc`: 退出搜索，恢复完整列表
- MVP 阶段：搜索框 UI 存在，输入后显示 "Search not implemented yet"

### 状态栏

- 显示当前 CF 名
- 显示估算 key 数量（使用 `rocksdb.estimate-num-keys`）
- 显示当前解析格式

## 解析器系统

### 支持的格式

| 格式 | Key 解析 | Value 解析 | 配置方式 |
|------|----------|------------|----------|
| `string` | UTF-8 字符串 | UTF-8 字符串 | 无需额外配置 |
| `hex` | 十六进制显示 | 十六进制显示 | 默认 fallback |
| `u64_be` | 大端 u64 | - | 无需额外配置 |
| `u64_le` | 小端 u64 | - | 无需额外配置 |
| `json` | - | 格式化 JSON | 无需额外配置 |
| `msgpack` | - | 解码后显示为 JSON | 无需额外配置 |
| `protobuf` | - | 解码后显示为 JSON | 需指定 proto_file + proto_message |

### 解析失败处理

- 解析失败时自动 fallback 到 hex 显示
- 在 Value 面板顶部显示警告：`⚠ Parse failed, showing hex`

## 模块结构

```
rocksdb-tui/
├── Cargo.toml
├── src/
│   ├── main.rs              # 入口、CLI 解析
│   ├── app.rs               # 应用状态管理
│   ├── db/
│   │   ├── mod.rs
│   │   └── secondary.rs     # Secondary 模式连接、迭代器封装
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── key.rs           # Key 解析器 (string/hex/u64)
│   │   └── value.rs         # Value 解析器 (json/hex/string)
│   ├── config.rs            # TOML 配置解析
│   └── ui/
│       ├── mod.rs
│       ├── layout.rs        # 三栏布局
│       ├── cf_list.rs       # Column Family 列表组件
│       ├── key_list.rs      # Key 列表组件（含分页）
│       └── value_view.rs    # Value 展示组件
```

## 核心依赖

```toml
[dependencies]
rocksdb = "0.22"             # RocksDB 绑定
ratatui = "0.28"             # TUI 框架
crossterm = "0.28"           # 终端控制
clap = { version = "4", features = ["derive"] }  # CLI 解析
serde = { version = "1", features = ["derive"] }
serde_json = "1"             # JSON 解析
toml = "0.8"                 # 配置文件解析
anyhow = "1"                 # 错误处理
```

## MVP 功能清单

- [ ] 命令行：`--db` 必填，`--config` 可选
- [ ] Secondary 模式连接本地 RocksDB
- [ ] 三栏 TUI 界面（CF 列表 / Key 列表 / Value 展示）
- [ ] 基于游标的分页加载（无限滚动）
- [ ] 解析器：`string`, `hex`, `json`
- [ ] 键盘导航：`j/k`, `Tab`, `gg/G`, `q`, `?`
- [ ] 搜索框 UI placeholder（不实现功能）
- [ ] 状态栏显示 CF 名、估算 key 数、格式

## 后续迭代

| 优先级 | 功能 |
|--------|------|
| P1 | 搜索功能（精确匹配 + 前缀搜索） |
| P1 | MessagePack 解析器 |
| P1 | Protobuf 解析器 |
| P2 | Key 格式：`u64_be`, `u64_le` |
| P2 | 刷新数据（catch up secondary） |
| P3 | Value 内容搜索/高亮 |
| P3 | 导出选中数据为 JSON 文件 |

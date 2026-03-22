# Plugin Management UI

**Date:** 2026-03-22
**Status:** Draft
**Scope:** Framework (synaptic-config) + Backend (gateway RPC) + Frontend (web/)
**Breaking:** No

## Problem

插件系统（P1/P2/P3）已完成后端，但 Web UI 无法查看、管理插件。用户无法看到哪些插件在运行、它们注册了什么 tools/hooks，也无法启用/禁用插件或控制 service 生命周期。

## Design

### 页面位置

新增 **Plugins** 页面，放在 Sidebar "Control" 分组（与 Overview/Channels/Sessions/Instances 并列）。

Tab 定义：
```typescript
{ key: "plugins", i18nKey: "dashboard.plugins.title", icon: Puzzle }
```

### 布局

**网格/列表可切换 + 右侧抽屉**

- 顶部操作栏：标题 + 视图切换按钮（Grid/List）+ "+ Install" 按钮
- 主区域：默认卡片网格视图，可切换为紧凑列表视图
  - 网格：`grid-template-columns: repeat(auto-fill, minmax(280px, 1fr))`
  - 列表：每行一个插件，水平排列信息
- 视图偏好持久化到 `localStorage`（key: `synapse:plugins-view`）
- 点击卡片/行 → 右侧滑出详情抽屉（约占 40% 宽度，窄屏时全宽覆盖）
- 抽屉打开时主区域压缩（不是遮罩），Escape 关闭

### 卡片视图内容

```
┌──────────────────────────────┐
│  memory-viking           ●  │  ← 名称 + 状态灯(绿/灰/红)
│  OpenViking memory provider  │  ← 描述
│  v0.2.0                     │  ← 版本
│                              │
│  [memory] [tools] [services] │  ← capabilities 标签
│                       builtin│  ← 来源 (builtin/workspace/global)
└──────────────────────────────┘
```

### 列表视图内容

```
● memory-viking    v0.2.0  OpenViking memory provider  [memory][tools][services]  builtin  ● Healthy
● builtin-tracing  v0.2.0  Agent tracing and latency   [hooks]                    builtin  ● Active
○ memory-native    v0.2.0  Native LTM with embeddings  [memory][tools]            builtin  ○ Disabled
```

状态灯颜色：
- `--success` (#30D158)：active + healthy
- `--text-tertiary` (#8E8E93)：disabled
- `--error` (#FF453A)：active but unhealthy / error

### 右侧抽屉

```
┌────────────────────────────────────┐
│  memory-viking              ✕     │ ← 关闭按钮 (Escape 也可关闭)
│  v0.2.0 · Slot: Memory            │
│  ● Healthy          [Enabled ◉]   │ ← 状态 + 启用开关
├────────────────────────────────────┤
│  REGISTERED TOOLS                  │
│  ├ memory_search                   │
│  ├ memory_read                     │
│  └ memory_commit                   │
├────────────────────────────────────┤
│  SERVICES                          │
│  ● VikingService    [Stop]         │ ← 带 start/stop 按钮 + 确认
├────────────────────────────────────┤
│  INTERCEPTORS                      │
│  └ MemoryRecallInterceptor         │
├────────────────────────────────────┤
│  EVENT SUBSCRIBERS                 │
│  └ MemoryCaptureSubscriber         │
├────────────────────────────────────┤
│  [Uninstall]                       │ ← builtin 插件隐藏此按钮
└────────────────────────────────────┘
```

### UI 状态

- **加载中**：使用 `LoadingSkeleton`（与其他 Dashboard 页面一致）
- **加载失败**：错误提示 + 重试按钮
- **空状态**："No external plugins. Built-in plugins are always available."
- **操作中**：按钮显示 spinner（toggle/start/stop/install/uninstall）
- **操作结果**：Toast 通知（成功/失败）
- **Builtin 保护**：builtin 插件隐藏 Uninstall 按钮，Disable 保留（可以禁用 tracing 等）
- **Service 控制确认**：Stop 操作需确认对话框

### 安装对话框

点击 "+ Install" → 弹出模态对话框：
- 输入框：本地路径（`/path/to/plugin/`），仅支持本地路径安装
- 确认/取消按钮
- 安装后刷新列表 + Toast 提示

---

## 后端 API

### 数据源变更

现有 `plugins.list` 从硬编码列表 + 文件系统扫描获取数据，**完全没有读取运行时 PluginRegistry**。需要切换为：

1. **主数据源**：`AppState.plugin_registry`（运行时已注册的插件）
2. **补充数据源**：文件系统扫描（发现但未加载的外部插件）

`handle_list` 需要通过 `ctx.state.plugin_registry` 访问 registry。

### PluginRegistry 查询能力（框架层变更）

当前 `PluginRegistry` 存储了 tools/services 但没有按 plugin 分组跟踪。需要在注册时记录映射。

**变更位置：`synaptic-config` crate（框架层）**

在 `PluginRegistry` 中新增：
```rust
/// Per-plugin registration tracking for introspection.
#[derive(Debug, Default, Clone, Serialize)]
pub struct PluginRegistrations {
    pub tools: Vec<String>,
    pub interceptors: Vec<String>,
    pub subscribers: Vec<String>,
    pub services: Vec<String>,
}

// PluginRegistry 新增字段
registrations: HashMap<String, PluginRegistrations>,
```

在 `PluginApi` 的每个 `register_*` 方法中，额外记录 name 到 `registrations` map：
- `register_tool` → 记录 `tool.name()` 到 `registrations[plugin_id].tools`
- `register_interceptor` → 记录类型名（`std::any::type_name`）
- `register_event_subscriber` → 使用 `EventSubscriber::name()` 方法（trait 已有此方法）
- `register_service` → 记录 `service.id()`

新增查询方法：
```rust
pub fn plugin_registrations(&self, plugin_id: &str) -> Option<&PluginRegistrations>;
pub fn all_registrations(&self) -> &HashMap<String, PluginRegistrations>;
```

### 启用/禁用状态

使用现有的 `PluginManager.disabled: Vec<String>` + `state.json` 持久化。`plugins.toggle` 调用 `PluginManager.enable(name)` 或 `PluginManager.disable(name)`。

注意：禁用需要重启生效（当前架构限制）。UI 显示提示："Plugin will be disabled after restart."

### Health 状态推导

```
无 service → "unknown"
所有 service.health_check() == true → "healthy"
部分 service 失败 → "degraded"
全部 service 失败 → "error"
插件已禁用 → "disabled"（前端用，不在 health 字段）
```

### 扩展 `plugins.list` 返回

```typescript
interface PluginInfo {
  name: string;
  version: string;
  description: string;
  author?: string;
  source: "builtin" | "workspace" | "global";
  enabled: boolean;
  slot?: string;
  capabilities: string[];
  health: "healthy" | "degraded" | "error" | "unknown";
  tools: string[];
  interceptors: string[];
  subscribers: string[];
  services: ServiceInfo[];
}

interface ServiceInfo {
  id: string;
  status: "running" | "stopped" | "error";
}
```

### 新增 RPC 方法

**`plugins.toggle`**
```json
{ "method": "plugins.toggle", "params": { "name": "memory-viking", "enabled": true } }
→ { "ok": true, "name": "memory-viking", "enabled": true, "message": "Takes effect after restart" }
```

**`plugins.service_control`**
```json
{ "method": "plugins.service_control", "params": { "plugin": "memory-viking", "service": "viking", "action": "stop" } }
→ { "ok": true, "service": "viking", "status": "stopped" }
```

---

## 实现范围

### 框架层 (synaptic)
| 文件 | 职责 |
|------|------|
| `synaptic-config/src/plugin/registry.rs` | 新增 `PluginRegistrations` + `registrations` 字段 + 查询方法 |
| `synaptic-config/src/plugin/plugin_api.rs` | 每个 `register_*` 方法额外记录到 registrations map |

### 后端 (synapse)
| 文件 | 职责 |
|------|------|
| `src/gateway/rpc/plugins_rpc.rs` | 重写 list（读 registry）、新增 toggle/service_control |
| `src/gateway/api/dashboard/mod.rs` | 添加 `/api/dashboard/plugins` REST endpoint |

### 前端 (web/)
| 文件 | 职责 |
|------|------|
| `web/src/components/dashboard/PluginsPage.tsx` | 页面组件：网格/列表切换 + 抽屉 + 安装对话框 |
| `web/src/components/Dashboard.tsx` | 添加 plugins tab 到 TABS 和 SIDEBAR_SECTIONS |
| `web/src/hooks/useDashboardAPI.ts` | 添加 fetchPlugins/togglePlugin/controlService/installPlugin/removePlugin |
| `web/src/i18n/en.json` + `zh.json` | 插件相关翻译 key |

### i18n Keys

```json
{
  "dashboard": {
    "plugins": {
      "title": "Plugins",
      "title_zh": "插件",
      "install": "Install Plugin",
      "install_zh": "安装插件",
      "viewGrid": "Grid View",
      "viewGrid_zh": "网格视图",
      "viewList": "List View",
      "viewList_zh": "列表视图",
      "installDialog": {
        "title": "Install Plugin",
        "title_zh": "安装插件",
        "pathLabel": "Local Path",
        "pathLabel_zh": "本地路径",
        "pathPlaceholder": "/path/to/plugin/directory",
        "confirm": "Install",
        "confirm_zh": "安装",
        "cancel": "Cancel",
        "cancel_zh": "取消"
      },
      "status": {
        "healthy": "Healthy", "healthy_zh": "正常",
        "degraded": "Degraded", "degraded_zh": "部分异常",
        "error": "Error", "error_zh": "异常",
        "unknown": "Unknown", "unknown_zh": "未知",
        "disabled": "Disabled", "disabled_zh": "已禁用",
        "active": "Active", "active_zh": "运行中"
      },
      "detail": {
        "slot": "Slot", "slot_zh": "插槽",
        "capabilities": "Capabilities", "capabilities_zh": "能力",
        "source": "Source", "source_zh": "来源",
        "tools": "Registered Tools", "tools_zh": "已注册工具",
        "services": "Services", "services_zh": "服务",
        "interceptors": "Interceptors", "interceptors_zh": "拦截器",
        "subscribers": "Event Subscribers", "subscribers_zh": "事件订阅",
        "noTools": "No tools registered", "noTools_zh": "无已注册工具",
        "noServices": "No services", "noServices_zh": "无服务",
        "start": "Start", "start_zh": "启动",
        "stop": "Stop", "stop_zh": "停止",
        "stopConfirm": "Stop service \"{{name}}\"? This may affect active operations.",
        "stopConfirm_zh": "停止服务 \"{{name}}\"？这可能影响正在进行的操作。",
        "enable": "Enable", "enable_zh": "启用",
        "disable": "Disable", "disable_zh": "禁用",
        "uninstall": "Uninstall", "uninstall_zh": "卸载",
        "uninstallConfirm": "Uninstall plugin \"{{name}}\"?",
        "uninstallConfirm_zh": "确定卸载插件 \"{{name}}\"？",
        "restartRequired": "Takes effect after restart",
        "restartRequired_zh": "重启后生效"
      },
      "source": {
        "builtin": "Built-in", "builtin_zh": "内置",
        "workspace": "Workspace", "workspace_zh": "工作区",
        "global": "Global", "global_zh": "全局"
      },
      "empty": "No plugins installed",
      "empty_zh": "未安装插件",
      "builtinNote": "Built-in plugins are always available.",
      "builtinNote_zh": "内置插件始终可用。",
      "fetchError": "Failed to load plugins",
      "fetchError_zh": "加载插件列表失败",
      "toast": {
        "installed": "Plugin installed successfully",
        "installed_zh": "插件安装成功",
        "removed": "Plugin uninstalled",
        "removed_zh": "插件已卸载",
        "toggled": "Plugin {{action}}",
        "toggled_zh": "插件已{{action}}",
        "serviceControlled": "Service {{action}}",
        "serviceControlled_zh": "服务已{{action}}",
        "error": "Operation failed: {{message}}",
        "error_zh": "操作失败：{{message}}"
      }
    }
  }
}
```

注意：实际实现时 `_zh` 后缀的值放入 `zh.json`，非 `_zh` 的放入 `en.json`。上面合并展示仅为方便审阅。

## YAGNI

- 插件市场/在线搜索 — registry 未实现
- 插件配置编辑 — 通过 Config 页面编辑 synapse.toml
- 插件日志查看 — 通过 Logs 页面按 plugin name 过滤
- 热重载 — 禁用/启用需重启生效
- 拖拽排序/优先级调整
- Name-based 安装（registry placeholder）— 仅本地路径安装

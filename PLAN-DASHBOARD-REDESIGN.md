# Dashboard Redesign Plan — 对齐 OpenClaw

## 新侧边栏结构

```
聊天
  └── 聊天

控制
  ├── 概览
  ├── 频道
  ├── 实例
  ├── 会话
  ├── 用量
  ├── 定时任务
  └── 节点

代理
  ├── 代理
  └── Skills

设置
  ├── 通用配置        ← 原 ConfigPage 拆分
  ├── 通信            ← 新页面
  ├── 自动化          ← 新页面
  ├── 基础设施        ← 新页面
  ├── AI 与代理       ← 新页面
  ├── 日志
  └── 调试
```

**删除页面**：工作区（合并到 Agent 详情 Files Tab）
**移动页面**：节点（代理→控制），用量（监控→控制）
**删除分区**：监控（合并到控制）
**新增页面**：通信、自动化、基础设施、AI 与代理

---

## Phase 1: 侧边栏调整 + 页面移动

### Step 1.1: 调整侧边栏分区和页面归属
**文件**: `web/src/components/Dashboard.tsx`

改动：
- 删除 "监控" 分区
- "控制" 分区包含：概览、频道、实例、会话、用量、定时任务、节点
- "代理" 分区包含：代理、Skills
- "设置" 分区包含：通用配置、通信、自动化、基础设施、AI与代理、日志、调试
- 删除 "工作区" Tab

```typescript
export const SIDEBAR_SECTIONS = [
  { i18nKey: "dashboard.control", keys: ["overview", "channels", "instances", "sessions", "usage", "schedules", "nodes"] },
  { i18nKey: "dashboard.agent", keys: ["agents", "skills"] },
  { i18nKey: "dashboard.settings", keys: ["general-config", "communications", "automation", "infrastructure", "ai-agents", "logs", "debug"] },
];
```

### Step 1.2: 注册新 Tab keys
**文件**: `web/src/components/Dashboard.tsx`, `web/src/types/dashboard.ts`

新增 TabKey：
- `"general-config"` — 通用配置（复用 ConfigPage，传 filter 参数）
- `"communications"` — 通信
- `"automation"` — 自动化
- `"infrastructure"` — 基础设施
- `"ai-agents"` — AI 与代理

删除 TabKey：
- `"workspace"` — 工作区（合并到 Agent Files Tab）
- `"config"` — 替换为 `"general-config"`

### Step 1.3: 删除工作区独立页
**文件**: `web/src/components/Dashboard.tsx`

- 删除 WorkspacePage import
- 删除 workspace tab 路由
- Agent 详情 Files Tab 已有完整文件编辑功能

### Step 1.4: i18n 新增
**文件**: `web/src/i18n/en.json`, `web/src/i18n/zh.json`

```json
{
  "dashboard": {
    "generalConfig": "General",
    "communications": "Communications",
    "automation": "Automation",
    "infrastructure": "Infrastructure",
    "aiAgents": "AI & Agents"
  }
}
```

---

## Phase 2: 设置页面拆分

### 核心思路

OpenClaw 每个设置页的统一 UI：
- 顶部工具栏：Open | Reload | Save | Apply | Update
- 搜索框 + Tab 分类
- Form 模式（结构化表单）/ Raw 模式切换
- 每个字段有描述、标签（advanced/performance/security）

我们的策略：复用现有 ConfigPage 的 Form 编辑器组件，按主题过滤显示不同配置节。

### Step 2.1: 创建 SettingsShell 共享组件
**文件**: 新建 `web/src/components/dashboard/SettingsShell.tsx`

抽取 ConfigPage 的搜索 + Tab 导航 + Save/Reload 工具栏为共享组件：

```typescript
interface SettingsShellProps {
  title: string;
  description: string;
  tabs: { key: string; label: string }[];
  activeTab: string;
  onTabChange: (tab: string) => void;
  children: React.ReactNode;
  onSave: () => void;
  onReload: () => void;
  saving: boolean;
  hasChanges: boolean;
}
```

### Step 2.2: 通用配置页 (GeneralConfigPage)
**文件**: 新建 `web/src/components/dashboard/GeneralConfigPage.tsx`

Tabs: Settings | Environment | Authentication | Updates | Logging | Diagnostics | Secrets

映射到 TOML 配置节：
- Settings → `[serve]`, `[auth]`, `[rate_limit]`
- Environment → 环境变量展示
- Authentication → `[auth]`
- Updates → 版本信息
- Logging → `[logging]`
- Diagnostics → 健康检查
- Secrets → `[secrets]`

### Step 2.3: 通信页 (CommunicationsPage)
**文件**: 新建 `web/src/components/dashboard/CommunicationsPage.tsx`

Tabs: Communication | Channels | Messages | Broadcast | Audio

映射：
- Communication → DM policy 全局设置、消息处理
- Channels → 频道级配置（chunk mode、typing indicator）— 从 ChannelsPage 的配置编辑部分复用
- Messages → 消息模板、格式化规则
- Broadcast → 广播组管理（从 AgentsPage 底部移来）
- Audio → 语音/TTS 配置 `[voice]`

### Step 2.4: 自动化页 (AutomationPage)
**文件**: 新建 `web/src/components/dashboard/AutomationPage.tsx`

Tabs: Automation | Commands | Hooks | Bindings | Approvals

映射：
- Commands → `[command]` 自定义斜杠命令
- Hooks → Heartbeat `[heartbeat]` + 事件钩子
- Bindings → `[[bindings]]` 全局路由绑定编辑
- Approvals → 执行审批配置 `[tool_policy]`

### Step 2.5: 基础设施页 (InfrastructurePage)
**文件**: 新建 `web/src/components/dashboard/InfrastructurePage.tsx`

Tabs: Gateway | Web | Security | Docker

映射：
- Gateway → `[serve]` 端口、认证、网关配置
- Web → Vite proxy、CORS
- Security → `[security]` SSRF guard、secret masking
- Docker → `[docker]` 沙箱配置

### Step 2.6: AI 与代理页 (AiAgentsPage)
**文件**: 新建 `web/src/components/dashboard/AiAgentsPage.tsx`

Tabs: Agents | Models | Skills | Tools | Memory | Session

映射：
- Agents → `[agents]` 默认 agent、agent defaults
- Models → `[models]` 模型目录、`[providers]` 提供商
- Skills → `[skills]` skills 系统配置
- Tools → `[tool_policy]` 工具策略
- Memory → `[memory]` 记忆配置
- Session → `[session]` 会话管理配置

---

## Phase 3: Agent 页面增强

### Step 3.1: 合并工作区到 Agent Files Tab
**文件**: `web/src/components/dashboard/AgentsPage.tsx`

增强现有 "Agent 文件" Tab：
- 复用 WorkspacePage 的完整文件编辑器（代码编辑器、保存、重置）
- 按 agent 加载对应 workspace 目录
- 支持新建/删除文件

### Step 3.2: Agent 定时任务 Tab
**文件**: `web/src/components/dashboard/AgentsPage.tsx`

新增 "Cron Jobs" Tab 到 Agent 详情：
- 过滤显示绑定到当前 agent 的定时任务
- 复用 SchedulesPage 的任务卡片组件

需要后端改动：
- ScheduleEntry 加 `agent` 字段
- 定时任务执行时路由到指定 agent

### Step 3.3: Agent 概览增强
**文件**: `web/src/components/dashboard/AgentsPage.tsx`

对齐 OpenClaw Agent Overview：
- Model Selection 下拉（可切换模型）
- Fallbacks 配置
- Skills Filter
- Reload Config / Save 按钮

---

## Phase 4: 聊天页 Agent 选择器

### Step 4.1: 聊天顶部 Agent 选择器
**文件**: `web/src/App.tsx` 或 聊天组件

在聊天区域顶部加 agent 下拉选择器：
- 列出所有可用 agent
- 选择后创建/切换到该 agent 的 session
- Session 下拉显示 `agent:{id}:{channel}:{peer}` 格式
- 类似 OC 的 session 选择器

---

## Phase 5: 配置编辑器增强

### Step 5.1: 字段描述和标签
**文件**: ConfigPage 相关组件

每个配置字段增加：
- 描述文本（从配置 schema 获取）
- 标签：`advanced` | `performance` | `security` | `network`
- 默认隐藏 advanced 字段，展开后显示

### Step 5.2: 全局搜索增强
**文件**: 各设置页面

搜索框支持：
- 按字段名搜索
- 按描述搜索
- 按标签过滤

---

## 执行顺序

```
Phase 1 (侧边栏):              ← 最先做，改动小但效果大
  1.1 侧边栏分区调整
  1.2 注册新 Tab keys
  1.3 删除工作区页
  1.4 i18n

Phase 2 (设置页面):             ← 工作量最大
  2.1 SettingsShell 共享组件
  2.2 通用配置页
  2.3 通信页
  2.4 自动化页
  2.5 基础设施页
  2.6 AI 与代理页

Phase 3 (Agent 增强):
  3.1 合并工作区
  3.2 Agent Cron Tab
  3.3 Agent 概览增强

Phase 4 (聊天):
  4.1 Agent 选择器

Phase 5 (编辑器增强):
  5.1 字段描述/标签
  5.2 搜索增强
```

## 验收标准

- [ ] 侧边栏 3 分区：控制 | 代理 | 设置
- [ ] "控制" 含 7 页：概览、频道、实例、会话、用量、定时任务、节点
- [ ] "代理" 含 2 页：代理、Skills
- [ ] "设置" 含 7 页：通用配置、通信、自动化、基础设施、AI与代理、日志、调试
- [ ] 工作区页面已删除，文件编辑在 Agent Files Tab
- [ ] 5 个新设置页各有 Tab 分类 + 搜索 + Form 编辑
- [ ] 通信页含 Broadcast 管理
- [ ] 自动化页含 Bindings + Commands + Approvals
- [ ] Agent 详情含 Cron Jobs Tab
- [ ] 聊天页有 Agent 选择器
- [ ] 所有 i18n 完整
- [ ] TypeScript + ESLint 通过

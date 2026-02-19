# 技能

技能通过向系统提示词注入特定领域的指令来扩展 Deep Agent 的行为。技能由一个 `SKILL.md` 文件定义，包含 YAML 前置元数据和 Markdown 格式的指令正文。`SkillsMiddleware` 从后端文件系统发现技能，并向代理展示索引，代理随后可以通过 `read_file` 工具按需读取完整的技能文件。

## SKILL.md 格式

每个技能文件以 `---` 标记之间的 YAML 前置元数据开头，包含 `name` 和 `description` 字段：

```markdown
---
name: search
description: Search the web for information
---

# Search Skill

Detailed instructions for how to perform web searches effectively...
```

前置元数据字段：

| 字段 | 是否必需 | 描述 |
|------|----------|------|
| `name` | 是 | 技能的唯一标识符 |
| `description` | 否 | 在技能索引中显示的单行摘要（省略时默认为空字符串） |

解析器通过扫描 `---` 标记之间的行，查找 `name:` 和 `description:` 前缀来提取字段值。值可以使用单引号或双引号。

## 技能目录结构

将技能文件放在工作区根目录的 `.skills/` 目录下：

```text
my-project/
  .skills/
    search/SKILL.md
    testing/SKILL.md
    documentation/SKILL.md
  src/
    main.rs
```

每个技能位于自己的子目录中。`SkillsMiddleware` 通过列出配置的 `skills_dir` 下的目录并从每个目录读取 `{skills_dir}/{dir}/SKILL.md` 来发现技能。

## 发现机制

`SkillsMiddleware` 实现了 `AgentMiddleware` trait。在每次调用 `before_model()` 时，它会：

1. 通过后端的 `ls()` 方法列出技能目录中的条目。
2. 对每个目录条目，读取 `{dir}/SKILL.md` 的前 50 行。
3. 解析 YAML 前置元数据以提取 `name` 和 `description`。
4. 构建 `<available_skills>` 部分并追加到系统提示词中。

注入的部分如下所示：

```text
<available_skills>
- **search**: Search the web for information (read `.skills/search/SKILL.md` for details)
- **testing**: Guidelines for writing tests (read `.skills/testing/SKILL.md` for details)
</available_skills>
```

代理看到此索引后，当需要详细指令时，可以通过 `read_file` 工具读取完整的 SKILL.md 文件。

## 配置

技能默认启用。通过 `DeepAgentOptions` 进行配置：

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend);
options.skills_dir = Some(".skills".to_string());  // 默认值
options.enable_skills = true;                       // 默认值
let agent = create_deep_agent(model, options)?;
```

要完全禁用技能，设置 `enable_skills = false`。要更改技能目录，将 `skills_dir` 设置为后端中的其他路径。

## 示例：添加 Rust 重构技能

在工作区中创建文件 `.skills/rust-refactoring/SKILL.md`：

```markdown
---
name: rust-refactoring
description: Best practices for refactoring Rust code
---

When refactoring Rust code, follow these guidelines:

1. Run `cargo clippy` before and after changes.
2. Prefer extracting functions over inline complexity.
3. Use `#[must_use]` on public functions that return values.
4. Write a test for every extracted function.
```

一旦该文件存在于后端中，`SkillsMiddleware` 会自动发现它并将其包含在系统提示词索引中。当代理遇到重构任务时，可以读取完整文件以获取详细指令。

没有编程方式的技能注册 API。所有技能都基于文件系统，在运行时通过扫描后端进行发现。

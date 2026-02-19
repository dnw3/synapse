# Skills

Skills extend a Deep Agent's behavior by injecting domain-specific instructions into the system prompt. A skill is defined by a `SKILL.md` file with YAML frontmatter and a body of Markdown instructions. The `SkillsMiddleware` discovers skills from the backend filesystem and presents an index to the agent, which can then read the full skill file on demand via the `read_file` tool.

## SKILL.md Format

Each skill file starts with YAML frontmatter between `---` markers containing `name` and `description` fields:

```markdown
---
name: search
description: Search the web for information
---

# Search Skill

Detailed instructions for how to perform web searches effectively...
```

The frontmatter fields:

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Unique identifier for the skill |
| `description` | no | One-line summary shown in the skill index (defaults to empty string if omitted) |

The parser extracts `name` and `description` by scanning lines between the `---` markers for `name:` and `description:` prefixes. Values may optionally be quoted with single or double quotes.

## Skills Directory Structure

Place skill files in a `.skills/` directory at the workspace root:

```text
my-project/
  .skills/
    search/SKILL.md
    testing/SKILL.md
    documentation/SKILL.md
  src/
    main.rs
```

Each skill lives in its own subdirectory. The `SkillsMiddleware` discovers them by listing directories under the configured `skills_dir` and reading `{skills_dir}/{dir}/SKILL.md` from each.

## How Discovery Works

The `SkillsMiddleware` implements the `AgentMiddleware` trait. On each call to `before_model()`, it:

1. Lists entries in the skills directory via the backend's `ls()` method.
2. For each directory entry, reads the first 50 lines of `{dir}/SKILL.md`.
3. Parses the YAML frontmatter to extract `name` and `description`.
4. Builds an `<available_skills>` section and appends it to the system prompt.

The injected section looks like:

```text
<available_skills>
- **search**: Search the web for information (read `.skills/search/SKILL.md` for details)
- **testing**: Guidelines for writing tests (read `.skills/testing/SKILL.md` for details)
</available_skills>
```

The agent sees this index and can read the full SKILL.md file via the `read_file` tool when it needs the detailed instructions.

## Configuration

Skills are enabled by default. Configure via `DeepAgentOptions`:

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend);
options.skills_dir = Some(".skills".to_string());  // default
options.enable_skills = true;                       // default
let agent = create_deep_agent(model, options)?;
```

To disable skills entirely, set `enable_skills = false`. To change the skills directory, set `skills_dir` to a different path within the backend.

## Example: Adding a Rust Refactoring Skill

Create the file `.skills/rust-refactoring/SKILL.md` in your workspace:

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

Once this file is present in the backend, the `SkillsMiddleware` will automatically discover it and include it in the system prompt index. The agent can then read the full file for detailed instructions when it encounters a refactoring task.

There is no programmatic skill registration API. All skills are filesystem-based, discovered at runtime by scanning the backend.

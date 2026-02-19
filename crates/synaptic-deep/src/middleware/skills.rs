use async_trait::async_trait;
use std::sync::Arc;
use synaptic_core::SynapticError;
use synaptic_middleware::{AgentMiddleware, ModelRequest};

use crate::backend::Backend;

/// A discovered skill with metadata parsed from YAML frontmatter.
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: String,
}

/// Middleware that discovers skills from the backend and injects an index into the system prompt.
///
/// Scans `{skills_dir}/*/SKILL.md` for YAML frontmatter containing `name` and `description`.
/// The agent can read the full SKILL.md via the `read_file` tool for detailed instructions.
pub struct SkillsMiddleware {
    backend: Arc<dyn Backend>,
    skills_dir: String,
}

impl SkillsMiddleware {
    pub fn new(backend: Arc<dyn Backend>, skills_dir: String) -> Self {
        Self {
            backend,
            skills_dir,
        }
    }

    async fn discover_skills(&self) -> Vec<Skill> {
        let entries = match self.backend.ls(&self.skills_dir).await {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut skills = Vec::new();
        for entry in entries {
            if !entry.is_dir {
                continue;
            }
            let skill_path = format!("{}/{}/SKILL.md", self.skills_dir, entry.name);
            if let Ok(content) = self.backend.read_file(&skill_path, 0, 50).await {
                if let Some(skill) = parse_skill_frontmatter(&content, &skill_path) {
                    skills.push(skill);
                }
            }
        }
        skills
    }
}

/// Parse YAML frontmatter between `---` markers to extract `name` and `description`.
fn parse_skill_frontmatter(content: &str, path: &str) -> Option<Skill> {
    let mut lines = content.lines();

    if lines.next()?.trim() != "---" {
        return None;
    }

    let mut name = None;
    let mut description = None;

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(val) = trimmed.strip_prefix("name:") {
            name = Some(
                val.trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            );
        } else if let Some(val) = trimmed.strip_prefix("description:") {
            description = Some(
                val.trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            );
        }
    }

    Some(Skill {
        name: name?,
        description: description.unwrap_or_default(),
        path: path.to_string(),
    })
}

#[async_trait]
impl AgentMiddleware for SkillsMiddleware {
    async fn before_model(&self, request: &mut ModelRequest) -> Result<(), SynapticError> {
        let skills = self.discover_skills().await;
        if skills.is_empty() {
            return Ok(());
        }

        let mut section = String::from("\n<available_skills>\n");
        for skill in &skills {
            section.push_str(&format!(
                "- **{}**: {} (read `{}` for details)\n",
                skill.name, skill.description, skill.path
            ));
        }
        section.push_str("</available_skills>\n");

        if let Some(ref mut prompt) = request.system_prompt {
            prompt.push_str(&section);
        } else {
            request.system_prompt = Some(section);
        }
        Ok(())
    }
}

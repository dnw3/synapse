use std::path::Path;

use axum::extract::{self, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};

use super::{read_config_file, OkResponse, ToggleResponse};
use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/skills", get(get_skills))
        .route("/dashboard/skills/{name}/toggle", post(toggle_skill))
        .route("/dashboard/skills/content", get(get_skill_content))
        .route("/dashboard/skills/files", get(get_skill_files))
        // Skill Store (ClawHub etc.)
        .route("/dashboard/store/search", get(store_search))
        .route("/dashboard/store/skills", get(store_list))
        .route("/dashboard/store/skills/{slug}", get(store_detail))
        .route(
            "/dashboard/store/skills/{slug}/files",
            get(store_skill_files),
        )
        .route(
            "/dashboard/store/skills/{slug}/files/{*path}",
            get(store_skill_file_content),
        )
        .route("/dashboard/store/install", post(store_install))
        .route("/dashboard/store/status", get(store_status))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/skills
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SkillResponse {
    name: String,
    path: String,
    source: String,
    description: String,
    user_invocable: bool,
    enabled: bool,
    eligible: bool,
    emoji: Option<String>,
    homepage: Option<String>,
    version: Option<String>,
    missing_env: Vec<String>,
    missing_bins: Vec<String>,
    has_install_specs: bool,
}

async fn get_skills(
    State(state): State<AppState>,
) -> Result<Json<Vec<SkillResponse>>, (StatusCode, String)> {
    let mut skills = Vec::new();

    let dirs: Vec<(&str, String)> = {
        let mut v = vec![("project", ".claude/skills".to_string())];
        if let Some(home) = dirs::home_dir() {
            v.push((
                "personal",
                home.join(".synapse/skills").to_string_lossy().to_string(),
            ));
            v.push((
                "personal",
                home.join(".claude/skills").to_string_lossy().to_string(),
            ));
        }
        v
    };

    let mut seen_names = std::collections::HashSet::new();

    for (source, dir_path) in dirs {
        if dir_path.is_empty() {
            continue;
        }
        let dir = Path::new(&dir_path);
        if !dir.exists() {
            continue;
        }
        if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    if !seen_names.insert(name.clone()) {
                        continue;
                    }

                    let (
                        description,
                        user_invocable,
                        eligible,
                        emoji,
                        homepage,
                        version,
                        missing_env,
                        missing_bins,
                        has_install,
                    ) = parse_skill_full_info(&path).await;

                    let enabled = !state
                        .config
                        .skill_overrides
                        .get(&name)
                        .map(|o| !o.enabled)
                        .unwrap_or(false);

                    skills.push(SkillResponse {
                        name,
                        path: path.to_string_lossy().to_string(),
                        source: source.to_string(),
                        description,
                        user_invocable,
                        enabled,
                        eligible,
                        emoji,
                        homepage,
                        version,
                        missing_env,
                        missing_bins,
                        has_install_specs: has_install,
                    });
                }
            }

            // Also scan subdirectories for SKILL.md
            if let Ok(mut sub_entries) = tokio::fs::read_dir(dir).await {
                while let Ok(Some(sub_entry)) = sub_entries.next_entry().await {
                    let sub_path = sub_entry.path();
                    if !sub_path.is_dir() {
                        continue;
                    }
                    let skill_md = sub_path.join("SKILL.md");
                    if !skill_md.exists() {
                        continue;
                    }

                    let name = sub_path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    if !seen_names.insert(name.clone()) {
                        continue;
                    }

                    let (
                        description,
                        user_invocable,
                        eligible,
                        emoji,
                        homepage,
                        version,
                        missing_env,
                        missing_bins,
                        has_install,
                    ) = parse_skill_full_info(&skill_md).await;

                    let enabled = !state
                        .config
                        .skill_overrides
                        .get(&name)
                        .map(|o| !o.enabled)
                        .unwrap_or(false);

                    skills.push(SkillResponse {
                        name,
                        path: skill_md.to_string_lossy().to_string(),
                        source: source.to_string(),
                        description,
                        user_invocable,
                        enabled,
                        eligible,
                        emoji,
                        homepage,
                        version,
                        missing_env,
                        missing_bins,
                        has_install_specs: has_install,
                    });
                }
            }
        }
    }

    Ok(Json(skills))
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/skills/{name}/toggle
// ---------------------------------------------------------------------------

async fn toggle_skill(
    State(state): State<AppState>,
    extract::Path(name): extract::Path<String>,
) -> Result<Json<ToggleResponse>, (StatusCode, String)> {
    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    let current_enabled = !state
        .config
        .skill_overrides
        .get(&name)
        .map(|o| !o.enabled)
        .unwrap_or(false);
    let new_enabled = !current_enabled;

    let root = doc.as_table_mut().unwrap();
    let overrides = root
        .entry("skill_overrides")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

    if let toml::Value::Table(tbl) = overrides {
        let skill_entry = tbl
            .entry(&name)
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if let toml::Value::Table(skill_tbl) = skill_entry {
            skill_tbl.insert("enabled".to_string(), toml::Value::Boolean(new_enabled));
        }
    }

    let new_content = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize: {}", e),
        )
    })?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    Ok(Json(ToggleResponse {
        enabled: new_enabled,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/skills/content
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SkillContentQuery {
    path: String,
}

#[derive(Serialize)]
struct SkillContentResponse {
    content: String,
}

async fn get_skill_content(
    Query(query): Query<SkillContentQuery>,
) -> Result<Json<SkillContentResponse>, (StatusCode, String)> {
    let path = Path::new(&query.path);

    let path_str = path.to_string_lossy();
    if path_str.contains("..") {
        return Err((StatusCode::FORBIDDEN, "invalid path".to_string()));
    }

    let canonical = path
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, "path not found".to_string()))?;
    let canonical_str = canonical.to_string_lossy();
    let home = dirs::home_dir().unwrap_or_default();
    let allowed_dirs = [
        home.join(".claude").join("skills"),
        home.join(".claude").join("commands"),
        home.join(".synapse").join("skills"),
    ];
    let is_skill_path = allowed_dirs.iter().any(|d| {
        d.exists()
            && d.canonicalize()
                .map(|cd| canonical_str.starts_with(&*cd.to_string_lossy()))
                .unwrap_or(false)
    });
    if !is_skill_path {
        return Err((
            StatusCode::FORBIDDEN,
            "only skill files can be read".to_string(),
        ));
    }

    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("read skill: {}", e)))?;

    let is_md = path_str.ends_with(".md");
    let content = if is_md && raw.starts_with("---") {
        if let Some(end) = raw[3..].find("\n---") {
            raw[3 + end + 4..].trim_start_matches('\n').to_string()
        } else {
            raw
        }
    } else {
        raw
    };

    let content = if content.len() > 65536 {
        format!("{}...\n\n[truncated at 64KB]", &content[..65536])
    } else {
        content
    };

    Ok(Json(SkillContentResponse { content }))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/skills/files
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SkillFilesQuery {
    path: String,
}

#[derive(Serialize)]
struct SkillFileEntry {
    name: String,
    size: u64,
}

#[derive(Serialize)]
struct SkillFilesListResponse {
    files: Vec<SkillFileEntry>,
}

async fn get_skill_files(
    Query(query): Query<SkillFilesQuery>,
) -> Result<Json<SkillFilesListResponse>, (StatusCode, String)> {
    let path = Path::new(&query.path);

    let skill_dir = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    let dir_str = skill_dir.to_string_lossy();
    if dir_str.contains("..") {
        return Err((StatusCode::FORBIDDEN, "invalid path".to_string()));
    }

    let canonical = skill_dir
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, "path not found".to_string()))?;
    let canonical_str = canonical.to_string_lossy();
    let home = dirs::home_dir().unwrap_or_default();
    let allowed_dirs = [
        home.join(".claude").join("skills"),
        home.join(".claude").join("commands"),
        home.join(".synapse").join("skills"),
    ];
    let is_skill_path = allowed_dirs.iter().any(|d| {
        d.exists()
            && d.canonicalize()
                .map(|cd| canonical_str.starts_with(&*cd.to_string_lossy()))
                .unwrap_or(false)
    });
    if !is_skill_path {
        return Err((
            StatusCode::FORBIDDEN,
            "only skill directories can be listed".to_string(),
        ));
    }

    if !skill_dir.exists() || !skill_dir.is_dir() {
        return Ok(Json(SkillFilesListResponse { files: vec![] }));
    }

    let mut files = Vec::new();
    collect_skill_files(skill_dir, skill_dir, &mut files, 0);
    files.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Json(SkillFilesListResponse { files }))
}

fn collect_skill_files(base: &Path, dir: &Path, out: &mut Vec<SkillFileEntry>, depth: usize) {
    if depth > 5 || out.len() > 500 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() > 500 {
            return;
        }
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_skill_files(base, &path, out, depth + 1);
        } else {
            let rel = path
                .strip_prefix(base)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.to_string());
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(SkillFileEntry { name: rel, size });
        }
    }
}

// ---------------------------------------------------------------------------
// Skill parsing helpers
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity)]
async fn parse_skill_full_info(
    path: &Path,
) -> (
    String,
    bool,
    bool,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
    bool,
) {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => {
            return (
                String::new(),
                true,
                true,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                false,
            )
        }
    };

    let content = content.trim_start_matches('\u{feff}');
    let mut lines = content.lines();

    if lines.next().map(|l| l.trim()) != Some("---") {
        return (
            String::new(),
            true,
            true,
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            false,
        );
    }

    let mut fm_lines = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        fm_lines.push(line);
    }

    let yaml_str = fm_lines.join("\n");
    let yaml: serde_json::Value = match serde_yml::from_str(&yaml_str) {
        Ok(v) => v,
        Err(_) => {
            return (
                String::new(),
                true,
                true,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                false,
            )
        }
    };
    let map = match yaml.as_object() {
        Some(m) => m,
        None => {
            return (
                String::new(),
                true,
                true,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                false,
            )
        }
    };

    let description = map
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let user_invocable = map
        .get("user-invocable")
        .or_else(|| map.get("user_invocable"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let version = map
        .get("version")
        .and_then(|v| v.as_str())
        .map(String::from);

    let oc = map
        .get("metadata")
        .and_then(|m| m.get("openclaw").or_else(|| m.get("clawdbot")));
    let homepage = map
        .get("homepage")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            oc.and_then(|o| o.get("homepage"))
                .and_then(|v| v.as_str())
                .map(String::from)
        });
    let emoji = map
        .get("emoji")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            oc.and_then(|o| o.get("emoji"))
                .and_then(|v| v.as_str())
                .map(String::from)
        });
    let has_install = oc
        .and_then(|o| o.get("install"))
        .and_then(|i| i.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    // Check requirements
    let mut missing_env = Vec::new();
    let required_env: Vec<String> = map
        .get("required-env")
        .or_else(|| map.get("required_env"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    for e in &required_env {
        if std::env::var(e).is_err() {
            missing_env.push(e.clone());
        }
    }

    let mut missing_bins = Vec::new();
    let required_bins: Vec<String> = map
        .get("required-bins")
        .or_else(|| map.get("required_bins"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    for b in &required_bins {
        if which::which(b).is_err() {
            missing_bins.push(b.clone());
        }
    }

    let eligible = missing_env.is_empty() && missing_bins.is_empty();

    (
        description,
        user_invocable,
        eligible,
        emoji,
        homepage,
        version,
        missing_env,
        missing_bins,
        has_install,
    )
}

#[allow(dead_code)]
async fn parse_skill_frontmatter(path: &Path) -> (String, bool) {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => return (String::new(), false),
    };

    let mut description = String::new();
    let mut user_invocable = false;

    if let Some(rest) = content.strip_prefix("---") {
        if let Some(end) = rest.find("---") {
            let frontmatter = &rest[..end];
            for line in frontmatter.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix("description:") {
                    description = val.trim().trim_matches('"').to_string();
                }
                if let Some(val) = line.strip_prefix("user-invocable:") {
                    user_invocable = val.trim() == "true";
                }
            }
        }
    }

    (description, user_invocable)
}

// ---------------------------------------------------------------------------
// ClawHub integration
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct HubSearchQuery {
    q: String,
    #[serde(default = "default_hub_limit")]
    limit: usize,
}

#[derive(Deserialize)]
struct HubListQuery {
    #[serde(default = "default_hub_limit")]
    limit: usize,
    sort: Option<String>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct HubInstallBody {
    slug: String,
    version: Option<String>,
}

fn default_hub_limit() -> usize {
    20
}

async fn store_search(
    State(state): State<AppState>,
    Query(query): Query<HubSearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let results = hub
        .search(&query.q, query.limit)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("store search: {}", e)))?;
    Ok(Json(
        serde_json::json!({ "results": results, "source": "clawhub" }),
    ))
}

async fn store_list(
    State(state): State<AppState>,
    Query(query): Query<HubListQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let items = hub
        .list(query.limit, query.sort.as_deref(), query.cursor.as_deref())
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("store list: {}", e)))?;
    Ok(Json(
        serde_json::json!({ "items": items, "source": "clawhub" }),
    ))
}

async fn store_detail(
    State(state): State<AppState>,
    extract::Path(slug): extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let detail = hub
        .detail(&slug)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("store detail: {}", e)))?;
    Ok(Json(detail))
}

async fn store_skill_files(
    State(state): State<AppState>,
    extract::Path(slug): extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let files_resp = hub
        .skill_files(&slug)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("store files: {}", e)))?;
    Ok(Json(serde_json::json!({
        "files": files_resp.files,
        "skillMd": files_resp.skill_md,
    })))
}

async fn store_skill_file_content(
    State(state): State<AppState>,
    extract::Path((slug, path)): extract::Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let content = hub.skill_file_content(&slug, &path).await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("store file content: {}", e),
        )
    })?;
    Ok(Json(serde_json::json!({
        "content": content,
    })))
}

async fn store_install(
    State(state): State<AppState>,
    Json(body): Json<HubInstallBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    crate::hub::install::install_from_hub(&hub, &body.slug, body.version.as_deref(), false)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("install: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn store_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let configured = hub.is_configured();
    let lock = crate::hub::install::read_lock_file();
    let installed_count = lock.skills.len();
    let installed: Vec<String> = lock.skills.keys().cloned().collect();
    Json(serde_json::json!({
        "configured": configured,
        "installedCount": installed_count,
        "installed": installed,
        "source": "clawhub",
    }))
}

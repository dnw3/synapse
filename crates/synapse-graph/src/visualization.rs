use std::fmt;
use std::io::Write;
use std::path::Path;

use synapse_core::SynapseError;

use crate::compiled::CompiledGraph;
use crate::state::State;
use crate::{END, START};

impl<S: State> CompiledGraph<S> {
    /// Render the graph as a Mermaid flowchart string.
    ///
    /// - `__start__` and `__end__` are rendered as rounded nodes `([...])`
    /// - User nodes are rendered as rectangles `[...]`
    /// - Fixed edges use solid arrows `-->`
    /// - Conditional edges with path_map use dashed arrows `-.->` with labels
    /// - Conditional edges without path_map emit a Mermaid comment
    pub fn draw_mermaid(&self) -> String {
        let mut lines = vec!["graph TD".to_string()];

        // Collect all node names (sorted for determinism)
        let mut node_names: Vec<&str> = self.nodes.keys().map(|s| s.as_str()).collect();
        node_names.sort();

        // Node definitions
        lines.push(format!("    {START}([\"{START}\"])"));
        for name in &node_names {
            lines.push(format!("    {name}[\"{name}\"]"));
        }
        lines.push(format!("    {END}([\"{END}\"])"));

        // Entry edge from START
        lines.push(format!("    {START} --> {}", self.entry_point));

        // Fixed edges (sorted for determinism)
        let mut fixed: Vec<(&str, &str)> = self
            .edges
            .iter()
            .map(|e| (e.source.as_str(), e.target.as_str()))
            .collect();
        fixed.sort();
        for (source, target) in fixed {
            lines.push(format!("    {source} --> {target}"));
        }

        // Conditional edges (sorted by source for determinism)
        let mut cond_sources: Vec<&str> = self
            .conditional_edges
            .iter()
            .map(|ce| ce.source.as_str())
            .collect();
        cond_sources.sort();

        for source in cond_sources {
            let ce = self
                .conditional_edges
                .iter()
                .find(|ce| ce.source == source)
                .unwrap();
            match &ce.path_map {
                Some(path_map) => {
                    let mut entries: Vec<(&String, &String)> = path_map.iter().collect();
                    entries.sort_by_key(|(label, _)| label.to_string());
                    for (label, target) in entries {
                        lines.push(format!("    {source} -.-> |{label}| {target}"));
                    }
                }
                None => {
                    lines.push(format!(
                        "    %% {source} has conditional edge (path_map not provided)"
                    ));
                }
            }
        }

        lines.join("\n")
    }

    /// Render the graph as a simple ASCII text summary.
    pub fn draw_ascii(&self) -> String {
        let mut lines = vec!["Graph:".to_string()];

        // Nodes (sorted)
        let mut node_names: Vec<&str> = self.nodes.keys().map(|s| s.as_str()).collect();
        node_names.sort();
        lines.push(format!("  Nodes: {}", node_names.join(", ")));

        // Entry
        lines.push(format!("  Entry: {START} -> {}", self.entry_point));

        // Edges
        lines.push("  Edges:".to_string());

        // Fixed edges (sorted)
        let mut fixed: Vec<(&str, &str)> = self
            .edges
            .iter()
            .map(|e| (e.source.as_str(), e.target.as_str()))
            .collect();
        fixed.sort();
        for (source, target) in fixed {
            lines.push(format!("    {source} -> {target}"));
        }

        // Conditional edges (sorted by source)
        let mut cond_sources: Vec<&str> = self
            .conditional_edges
            .iter()
            .map(|ce| ce.source.as_str())
            .collect();
        cond_sources.sort();

        for source in cond_sources {
            let ce = self
                .conditional_edges
                .iter()
                .find(|ce| ce.source == source)
                .unwrap();
            match &ce.path_map {
                Some(path_map) => {
                    let mut targets: Vec<&String> = path_map.values().collect();
                    targets.sort();
                    targets.dedup();
                    let targets_str = targets.iter().map(|t| t.as_str()).collect::<Vec<_>>();
                    lines.push(format!(
                        "    {source} -> {}  [conditional]",
                        targets_str.join(" | ")
                    ));
                }
                None => {
                    lines.push(format!("    {source} -> ???  [conditional]"));
                }
            }
        }

        lines.join("\n")
    }

    /// Render the graph in Graphviz DOT format.
    pub fn draw_dot(&self) -> String {
        let mut lines = vec!["digraph G {".to_string()];
        lines.push("    rankdir=TD;".to_string());

        // Node definitions (sorted)
        let mut node_names: Vec<&str> = self.nodes.keys().map(|s| s.as_str()).collect();
        node_names.sort();

        lines.push(format!("    \"{START}\" [shape=oval];"));
        for name in &node_names {
            lines.push(format!("    \"{name}\" [shape=box];"));
        }
        lines.push(format!("    \"{END}\" [shape=oval];"));

        // Entry edge
        lines.push(format!(
            "    \"{START}\" -> \"{}\" [style=solid];",
            self.entry_point
        ));

        // Fixed edges (sorted)
        let mut fixed: Vec<(&str, &str)> = self
            .edges
            .iter()
            .map(|e| (e.source.as_str(), e.target.as_str()))
            .collect();
        fixed.sort();
        for (source, target) in fixed {
            lines.push(format!("    \"{source}\" -> \"{target}\" [style=solid];"));
        }

        // Conditional edges (sorted)
        let mut cond_sources: Vec<&str> = self
            .conditional_edges
            .iter()
            .map(|ce| ce.source.as_str())
            .collect();
        cond_sources.sort();

        for source in cond_sources {
            let ce = self
                .conditional_edges
                .iter()
                .find(|ce| ce.source == source)
                .unwrap();
            if let Some(ref path_map) = ce.path_map {
                let mut entries: Vec<(&String, &String)> = path_map.iter().collect();
                entries.sort_by_key(|(label, _)| label.to_string());
                for (label, target) in entries {
                    lines.push(format!(
                        "    \"{source}\" -> \"{target}\" [style=dashed, label=\"{label}\"];",
                    ));
                }
            }
        }

        lines.push("}".to_string());
        lines.join("\n")
    }

    /// Render the Mermaid diagram as an image via the mermaid.ink API.
    ///
    /// Requires internet access. The generated Mermaid text is URL-safe base64-encoded
    /// and sent to `https://mermaid.ink/img/{encoded}`. The image (JPEG format) is
    /// written to the specified file path.
    ///
    /// Note: mermaid.ink returns JPEG from the `/img/` endpoint. For SVG output,
    /// use [`draw_mermaid_svg`](Self::draw_mermaid_svg) instead.
    pub async fn draw_mermaid_png(&self, path: impl AsRef<Path>) -> Result<(), SynapseError> {
        self.fetch_mermaid_ink("img", path).await
    }

    /// Render the Mermaid diagram as an SVG image via the mermaid.ink API.
    ///
    /// Requires internet access. The generated Mermaid text is URL-safe base64-encoded
    /// and sent to `https://mermaid.ink/svg/{encoded}`. The SVG response is written
    /// to the specified file path.
    pub async fn draw_mermaid_svg(&self, path: impl AsRef<Path>) -> Result<(), SynapseError> {
        self.fetch_mermaid_ink("svg", path).await
    }

    async fn fetch_mermaid_ink(
        &self,
        endpoint: &str,
        path: impl AsRef<Path>,
    ) -> Result<(), SynapseError> {
        use base64::Engine;

        let mermaid = self.draw_mermaid();
        let encoded = base64::engine::general_purpose::URL_SAFE.encode(mermaid.as_bytes());
        let url = format!("https://mermaid.ink/{endpoint}/{encoded}");

        let response = reqwest::get(&url)
            .await
            .map_err(|e| SynapseError::Graph(format!("mermaid.ink request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(SynapseError::Graph(format!(
                "mermaid.ink returned status {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            SynapseError::Graph(format!("failed to read mermaid.ink response: {e}"))
        })?;

        std::fs::write(path, &bytes)
            .map_err(|e| SynapseError::Graph(format!("failed to write image file: {e}")))?;

        Ok(())
    }

    /// Render the graph as a PNG image using the Graphviz `dot` command.
    ///
    /// Requires `dot` (Graphviz) to be installed and available in `$PATH`.
    /// The DOT output is piped to `dot -Tpng` and the resulting PNG is written
    /// to the specified file path.
    pub fn draw_png(&self, path: impl AsRef<Path>) -> Result<(), SynapseError> {
        let dot = self.draw_dot();

        let mut child = std::process::Command::new("dot")
            .args(["-Tpng"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                SynapseError::Graph(format!(
                    "failed to run 'dot' command (is Graphviz installed?): {e}"
                ))
            })?;

        child
            .stdin
            .take()
            .unwrap()
            .write_all(dot.as_bytes())
            .map_err(|e| SynapseError::Graph(format!("failed to write to dot stdin: {e}")))?;

        let output = child
            .wait_with_output()
            .map_err(|e| SynapseError::Graph(format!("dot command failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SynapseError::Graph(format!("dot command failed: {stderr}")));
        }

        std::fs::write(path, &output.stdout)
            .map_err(|e| SynapseError::Graph(format!("failed to write PNG file: {e}")))?;

        Ok(())
    }
}

impl<S: State> fmt::Display for CompiledGraph<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.draw_ascii())
    }
}

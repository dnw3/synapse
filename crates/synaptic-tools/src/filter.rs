use std::collections::{HashMap, HashSet};

use synaptic_core::ToolDefinition;

/// Context available when filtering tools.
#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    /// Number of agent turns completed so far.
    pub turn_count: usize,
    /// Name of the last tool that was called, if any.
    pub last_tool: Option<String>,
    /// Arbitrary metadata for custom filter logic.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Trait for filtering available tools based on context.
pub trait ToolFilter: Send + Sync {
    /// Filter the list of tool definitions based on the current context.
    fn filter(&self, tools: Vec<ToolDefinition>, context: &FilterContext) -> Vec<ToolDefinition>;
}

/// Only allows tools whose names are in the allow list.
pub struct AllowListFilter {
    allowed: HashSet<String>,
}

impl AllowListFilter {
    pub fn new(allowed: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed: allowed.into_iter().map(|s| s.into()).collect(),
        }
    }
}

impl ToolFilter for AllowListFilter {
    fn filter(&self, tools: Vec<ToolDefinition>, _context: &FilterContext) -> Vec<ToolDefinition> {
        tools
            .into_iter()
            .filter(|t| self.allowed.contains(&t.name))
            .collect()
    }
}

/// Removes tools whose names are in the deny list.
pub struct DenyListFilter {
    denied: HashSet<String>,
}

impl DenyListFilter {
    pub fn new(denied: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            denied: denied.into_iter().map(|s| s.into()).collect(),
        }
    }
}

impl ToolFilter for DenyListFilter {
    fn filter(&self, tools: Vec<ToolDefinition>, _context: &FilterContext) -> Vec<ToolDefinition> {
        tools
            .into_iter()
            .filter(|t| !self.denied.contains(&t.name))
            .collect()
    }
}

/// Filters tools based on state machine rules: which tools are allowed
/// after certain tools, and which tools become available after N turns.
pub struct StateMachineFilter {
    /// Rules keyed by the name of the last tool called.
    after_tool_rules: HashMap<String, HashSet<String>>,
    /// Rules that gate tools behind a turn count threshold.
    turn_thresholds: Vec<TurnThreshold>,
}

/// Rule for what tools to add after a certain number of turns.
#[derive(Debug, Clone)]
struct TurnThreshold {
    /// Minimum turn count for this rule to apply.
    min_turns: usize,
    /// Tools gated behind this threshold.
    add_tools: HashSet<String>,
}

impl StateMachineFilter {
    pub fn new() -> Self {
        Self {
            after_tool_rules: HashMap::new(),
            turn_thresholds: Vec::new(),
        }
    }

    /// Add a rule: after `tool_name` is called, only `allowed_next` tools are available.
    pub fn after_tool(
        mut self,
        tool_name: impl Into<String>,
        allowed_next: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.after_tool_rules.insert(
            tool_name.into(),
            allowed_next.into_iter().map(|s| s.into()).collect(),
        );
        self
    }

    /// Add a rule: tools in `add_tools` are hidden until `min_turns` is reached.
    pub fn turn_threshold(
        mut self,
        min_turns: usize,
        add_tools: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.turn_thresholds.push(TurnThreshold {
            min_turns,
            add_tools: add_tools.into_iter().map(|s| s.into()).collect(),
        });
        self
    }
}

impl Default for StateMachineFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolFilter for StateMachineFilter {
    fn filter(&self, tools: Vec<ToolDefinition>, context: &FilterContext) -> Vec<ToolDefinition> {
        let mut result = tools;

        // Apply after-tool rules: restrict to only allowed_next
        if let Some(last) = &context.last_tool {
            if let Some(allowed) = self.after_tool_rules.get(last) {
                result.retain(|t| allowed.contains(&t.name));
            }
        }

        // Apply turn thresholds: collect tools that are gated by turn count.
        // If a tool appears in ANY threshold, it is only included when that
        // threshold is met.
        let mut gated_tools: HashMap<&str, bool> = HashMap::new();
        for threshold in &self.turn_thresholds {
            let met = context.turn_count >= threshold.min_turns;
            for tool_name in &threshold.add_tools {
                let entry = gated_tools.entry(tool_name.as_str()).or_insert(false);
                if met {
                    *entry = true;
                }
            }
        }

        if !gated_tools.is_empty() {
            result.retain(|t| {
                match gated_tools.get(t.name.as_str()) {
                    Some(&met) => met, // Gated tool: only include if threshold met
                    None => true,      // Not gated: always include
                }
            });
        }

        result
    }
}

/// Composes multiple filters, applying them in sequence.
pub struct CompositeFilter(pub Vec<Box<dyn ToolFilter>>);

impl CompositeFilter {
    pub fn new(filters: Vec<Box<dyn ToolFilter>>) -> Self {
        Self(filters)
    }
}

impl ToolFilter for CompositeFilter {
    fn filter(
        &self,
        mut tools: Vec<ToolDefinition>,
        context: &FilterContext,
    ) -> Vec<ToolDefinition> {
        for f in &self.0 {
            tools = f.filter(tools, context);
        }
        tools
    }
}

use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use synaptic::core::Tool;
use synaptic::deep::DeepAgentOptions;
use synaptic::session::SessionManager;

/// Register all built-in tools, MCP tools, plugin tools, and session tools on
/// the `DeepAgentOptions`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn register_tools(
    options: &mut DeepAgentOptions,
    cwd: &Path,
    mcp_tools: Vec<Arc<dyn Tool>>,
    session_mgr: Option<&Arc<SessionManager>>,
    plugin_registry: Option<&Arc<RwLock<synaptic::plugin::PluginRegistry>>>,
    channel_registry: Option<&Arc<tokio::sync::RwLock<crate::gateway::messages::ChannelRegistry>>>,
) {
    // Add MCP tools
    options.tools.extend(mcp_tools);

    // Add plugin-registered tools
    if let Some(registry) = plugin_registry {
        let reg = registry.read().unwrap();
        for tool in reg.tools() {
            options.tools.push(tool.clone());
        }
        tracing::debug!(count = reg.tools().len(), "Plugin tools merged into agent");
    }

    // Add apply_patch tool
    options.tools.push(crate::tools::ApplyPatchTool::new(cwd));

    // Add PDF reading tool
    options.tools.push(crate::tools::ReadPdfTool::new(cwd));

    // Add Firecrawl web scraping tool
    options.tools.push(crate::tools::FirecrawlTool::new());

    // Add image analysis tool (always available — works with any vision model)
    options.tools.push(crate::tools::AnalyzeImageTool::new(cwd));

    // Add audio transcription tool
    #[cfg(feature = "voice")]
    {
        // Try to create an OpenAI STT provider from environment
        if let Ok(voice) = synaptic_integrations::voice::openai::OpenAiVoice::new("OPENAI_API_KEY")
        {
            let stt: Arc<dyn synaptic_integrations::voice::SttProvider> = Arc::new(voice);
            options
                .tools
                .push(crate::tools::TranscribeAudioTool::new(cwd, stt));
            tracing::info!("Audio transcription tool registered");
        }
    }

    // Memory tools (memory_search, memory_get, memory_save, memory_forget) are now
    // registered by the memory plugin via PluginApi — no direct registration here.

    // Add session tools if SessionManager is available
    if let Some(mgr) = session_mgr {
        options
            .tools
            .push(crate::tools::SessionsListTool::new(mgr.clone()));
        options
            .tools
            .push(crate::tools::SessionsHistoryTool::new(mgr.clone()));
        options
            .tools
            .push(crate::tools::SessionsSendTool::new(mgr.clone()));
        options
            .tools
            .push(crate::tools::SessionsSpawnTool::new(mgr.clone()));
    }

    // Add platform action tool (channel registry wired when running as gateway)
    {
        let tool = if let Some(reg) = channel_registry {
            crate::tools::PlatformActionTool::with_registry(reg.clone())
        } else {
            crate::tools::PlatformActionTool::new()
        };
        options.tools.push(Arc::new(tool));
        tracing::debug!(
            has_registry = channel_registry.is_some(),
            "PlatformActionTool registered"
        );
    }

    // Add browser tools if enabled
    #[cfg(feature = "browser")]
    {
        use synaptic::browser::{browser_tools, BrowserConfig};
        let browser_config = BrowserConfig::default();
        let tools = browser_tools(&browser_config);
        tracing::info!(tool_count = tools.len(), "Browser tools available");
        options.tools.extend(tools);
    }
}

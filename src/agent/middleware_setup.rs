use std::sync::Arc;

use async_trait::async_trait;
use synaptic::callbacks::CostTrackingCallback;
use synaptic::condenser::CondenserMiddleware;
use synaptic::condenser::{
    ChunkedSummarizingCondenser, Condenser, LlmSummarizingCondenser, TokenBudgetCondenser,
};
use synaptic::core::{ChatModel, SynapticError, TokenCounter};
use synaptic::deep::backend::Backend;
use synaptic::deep::DeepAgentOptions;
use synaptic::middleware::{
    CircuitBreakerConfig, CircuitBreakerMiddleware, Interceptor, ModelCaller, ModelRequest,
    ModelResponse, RiskLevel, RuleBasedAnalyzer, SecurityConfirmationCallback, SecurityMiddleware,
    SsrfGuardConfig, SsrfGuardMiddleware, ThresholdConfirmationPolicy,
};
use synaptic::secrets::SecretMaskingMiddleware;
use synaptic::secrets::SecretRegistry;

use crate::config::SynapseConfig;

use super::builder::SessionOverrides;
use super::callbacks::AutoApproveCallback;
use super::middleware::{build_fallback_interceptor, LoopDetectionMiddleware};
use super::thinking::{ThinkingMiddleware, VerboseMiddleware};
use synaptic::deep::AgentTracingMiddleware;

/// Lightweight interceptor that records token usage on every LLM response.
pub(crate) struct CostTrackingInterceptor(pub Arc<CostTrackingCallback>);

#[async_trait]
impl Interceptor for CostTrackingInterceptor {
    async fn wrap_model_call(
        &self,
        request: ModelRequest,
        next: &dyn ModelCaller,
    ) -> Result<ModelResponse, SynapticError> {
        let response = next.call(request).await?;
        match response.usage {
            Some(ref usage) => {
                tracing::debug!(
                    input = usage.input_tokens,
                    output = usage.output_tokens,
                    total = usage.total_tokens,
                    "Token usage"
                );
                self.0.record_usage(usage).await;
            }
            None => {
                tracing::debug!("Provider returned no usage data for this response");
            }
        }
        Ok(response)
    }
}

/// Set up the full interceptor stack on `options`, including tracing, secret masking,
/// SSRF guard, security, tool policy, circuit breaker, loop detection, thinking,
/// verbose, auto-compaction, deep summarization, cost tracking, OTel, and fallback.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn setup_middleware(
    options: &mut DeepAgentOptions,
    config: &SynapseConfig,
    model: &Arc<dyn ChatModel>,
    backend: &Arc<dyn Backend>,
    security_callback: Option<Arc<dyn SecurityConfirmationCallback>>,
    session_overrides: Option<&SessionOverrides>,
    cost_tracker: Option<Arc<CostTrackingCallback>>,
) {
    // Agent tracing middleware (first, so it captures the full picture)
    options
        .interceptors
        .push(Arc::new(AgentTracingMiddleware::new()));

    // Secret masking middleware
    setup_secret_masking(options, config);

    // SSRF guard middleware
    if config.security.as_ref().is_none_or(|s| s.ssrf_guard) {
        options.interceptors.push(Arc::new(SsrfGuardMiddleware::new(
            SsrfGuardConfig::default(),
        )));
        tracing::info!("SSRF guard enabled");
    }

    // Security middleware
    setup_security(options, config, security_callback);

    // Tool policy interceptor
    setup_tool_policy(options, config);

    // Circuit breaker middleware
    {
        let threshold = config
            .security
            .as_ref()
            .map(|s| s.circuit_breaker_threshold)
            .unwrap_or(5);
        let cb_config = CircuitBreakerConfig {
            failure_threshold: threshold,
            ..Default::default()
        };
        options
            .interceptors
            .push(Arc::new(CircuitBreakerMiddleware::new(cb_config)));
        tracing::info!(threshold = threshold, "Circuit breaker enabled");
    }

    // Loop detection interceptor
    {
        let max_repeats = 3;
        options
            .interceptors
            .push(Arc::new(LoopDetectionMiddleware::new(max_repeats)));
        tracing::info!(threshold = max_repeats, "Loop detection enabled");
    }

    // Thinking interceptor (session override or config default)
    setup_thinking(options, config, session_overrides);

    // Verbose interceptor (session override)
    setup_verbose(options, session_overrides);

    // Auto-compaction via CondenserMiddleware
    if config.memory.auto_compact {
        let condenser: Arc<dyn Condenser> = match config.memory.compact_strategy.as_str() {
            "summarize" => Arc::new(LlmSummarizingCondenser::new(
                model.clone(),
                config.memory.auto_compact_threshold,
                config.memory.keep_recent,
            )),
            "chunked" => Arc::new(ChunkedSummarizingCondenser::new(
                model.clone(),
                config.memory.auto_compact_threshold,
                config.memory.keep_recent,
                30,
            )),
            _ => {
                let counter: Arc<dyn TokenCounter> =
                    Arc::new(synaptic::core::HeuristicTokenCounter);
                Arc::new(TokenBudgetCondenser::new(
                    config.memory.auto_compact_threshold,
                    counter,
                ))
            }
        };
        let condenser_mw = CondenserMiddleware::new(condenser);
        options.interceptors.push(Arc::new(condenser_mw));
        tracing::info!(
            strategy = %config.memory.compact_strategy,
            threshold = config.memory.auto_compact_threshold,
            "Auto-compaction enabled"
        );
    }

    // DeepSummarizationMiddleware
    if config.memory.auto_compact {
        let summarization =
            synaptic::deep::middleware::summarization::DeepSummarizationMiddleware::new(
                backend.clone(),
                model.clone(),
                config.memory.auto_compact_threshold,
                0.8,
            );
        options.interceptors.push(Arc::new(summarization));
        tracing::info!("Deep summarization middleware enabled");
    }

    // Cost tracking interceptor — records token usage from every LLM response
    if let Some(tracker) = cost_tracker {
        let model_name = config.base.model.model.clone();
        tracker.set_model(&model_name).await;
        options
            .interceptors
            .push(Arc::new(CostTrackingInterceptor(tracker)));
        tracing::info!("Cost tracking interceptor enabled");
    }

    // OpenTelemetry event subscriber — register directly on EventBus
    #[cfg(feature = "otel")]
    {
        if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
            use synaptic::callbacks::OpenTelemetryCallback;
            let otel = Arc::new(OpenTelemetryCallback::new("synapse"));
            if let Some(ref bus) = options.event_bus {
                bus.subscribe(otel, 100, "otel");
                tracing::info!("OTel subscriber registered on EventBus");
            } else {
                tracing::warn!("OTel subscriber skipped: no EventBus configured");
            }
        }
    }

    // Fallback interceptor
    if let Some(fallback) = build_fallback_interceptor(config) {
        options.interceptors.push(Arc::new(fallback));
    }
}

/// Set up secret masking middleware.
fn setup_secret_masking(options: &mut DeepAgentOptions, config: &SynapseConfig) {
    if config.secrets.as_ref().is_none_or(|s| s.mask_api_keys) {
        let registry = Arc::new(SecretRegistry::new());

        if let Ok(api_key) = config.base.resolve_api_key() {
            if !api_key.is_empty() {
                registry.register("api_key", &api_key);
            }
        }

        if let Some(ref secrets_cfg) = config.secrets {
            for var_name in &secrets_cfg.additional_env_vars {
                if let Ok(val) = std::env::var(var_name) {
                    if !val.is_empty() {
                        registry.register(var_name, &val);
                    }
                }
            }
        }

        let secret_mw = SecretMaskingMiddleware::new(registry);
        options.interceptors.push(Arc::new(secret_mw));
        tracing::info!("Secret masking enabled");
    }
}

/// Set up security middleware.
fn setup_security(
    options: &mut DeepAgentOptions,
    config: &SynapseConfig,
    security_callback: Option<Arc<dyn SecurityConfirmationCallback>>,
) {
    if config.security.as_ref().is_none_or(|s| s.enabled) {
        let mut analyzer = RuleBasedAnalyzer::new()
            .with_default_risk(RiskLevel::Low)
            .with_tool_risk("bash", RiskLevel::High)
            .with_tool_risk("shell_exec", RiskLevel::High)
            .with_tool_risk("write_file", RiskLevel::Medium)
            .with_tool_risk("delete_file", RiskLevel::High);

        if let Some(ref sec_cfg) = config.security {
            for tool in &sec_cfg.high_risk_tools {
                analyzer = analyzer.with_tool_risk(tool.as_str(), RiskLevel::High);
            }
            for tool in &sec_cfg.blocked_tools {
                analyzer = analyzer.with_tool_risk(tool.as_str(), RiskLevel::Critical);
            }
        }

        let policy = Arc::new(ThresholdConfirmationPolicy::new(RiskLevel::High));
        let callback: Arc<dyn SecurityConfirmationCallback> =
            security_callback.unwrap_or_else(|| Arc::new(AutoApproveCallback));
        options.interceptors.push(Arc::new(SecurityMiddleware::new(
            Arc::new(analyzer),
            policy,
            callback,
        )));
        tracing::info!("Security middleware enabled");
    }
}

/// Set up tool policy interceptor if any policies are configured.
fn setup_tool_policy(options: &mut DeepAgentOptions, config: &SynapseConfig) {
    let tp = &config.tool_policy;
    let has_policy =
        !tp.owner_only_tools.is_empty() || !tp.tool_allow.is_empty() || !tp.tool_deny.is_empty();
    if has_policy {
        options
            .interceptors
            .push(Arc::new(super::tool_policy::ToolPolicyMiddleware::new(
                tp.clone(),
            )));
        tracing::info!("Tool policy interceptor enabled");
    }
}

/// Set up thinking interceptor from session overrides or config defaults.
fn setup_thinking(
    options: &mut DeepAgentOptions,
    config: &SynapseConfig,
    session_overrides: Option<&SessionOverrides>,
) {
    let thinking_level = session_overrides
        .and_then(|o| o.thinking.as_deref())
        .or_else(|| {
            // Check model entry for default thinking level
            if let Some(ref models) = config.model_catalog {
                let primary = &config.base.model.model;
                models
                    .iter()
                    .find(|m| m.name == *primary || m.aliases.contains(&primary.to_string()))
                    .and_then(|m| m.thinking.as_deref())
            } else {
                None
            }
        });

    if let Some(level) = thinking_level {
        if level == "adaptive" {
            options
                .interceptors
                .push(Arc::new(ThinkingMiddleware::adaptive()));
            tracing::info!("Adaptive thinking enabled");
        } else if let Some(tc) = super::thinking::parse_thinking_level(level) {
            options
                .interceptors
                .push(Arc::new(ThinkingMiddleware::new(Some(tc))));
            tracing::info!(level = %level, "Thinking mode enabled");
        }
    }
}

/// Set up verbose interceptor from session overrides.
fn setup_verbose(options: &mut DeepAgentOptions, session_overrides: Option<&SessionOverrides>) {
    let verbose_level = session_overrides.and_then(|o| o.verbose.as_deref());
    if let Some(level) = verbose_level {
        if level != "on" && level != "inherit" {
            options
                .interceptors
                .push(Arc::new(VerboseMiddleware::new(level)));
            tracing::info!(level = %level, "Verbose mode enabled");
        }
    }
}

/// Set up post-session reflection for self-evolution.
pub(crate) fn setup_reflection(
    options: &mut DeepAgentOptions,
    config: &SynapseConfig,
    model: &Arc<dyn ChatModel>,
) {
    if config.reflection.enabled {
        let reflection_model_name = config.reflection.model.as_deref();
        match reflection_model_name {
            Some(name) => match super::model::build_model_by_name(config, name) {
                Ok(m) => {
                    let rcfg = synaptic::deep::ReflectionConfig {
                        min_messages: config.reflection.min_messages,
                        memory_file: config.base.paths.memory_file.clone(),
                        ..Default::default()
                    };
                    options.reflection_model = Some(m);
                    options.reflection_config = Some(rcfg);
                    tracing::info!(model = name, "Reflection middleware enabled");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to build reflection model, skipping")
                }
            },
            None => {
                // Use main model (fallback)
                let rcfg = synaptic::deep::ReflectionConfig {
                    min_messages: config.reflection.min_messages,
                    memory_file: config.base.paths.memory_file.clone(),
                    ..Default::default()
                };
                options.reflection_model = Some(model.clone());
                options.reflection_config = Some(rcfg);
                tracing::info!("Reflection middleware enabled (using main model)");
            }
        }
    }
}

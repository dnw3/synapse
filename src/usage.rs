//! Usage tracking and cost display.

use std::sync::Arc;

use colored::Colorize;
use std::collections::HashMap;
use synaptic::callbacks::{default_pricing, CostTrackingCallback, ModelPricing, UsageSnapshot};

/// Create a CostTrackingCallback with default pricing.
pub fn create_tracker() -> Arc<CostTrackingCallback> {
    Arc::new(CostTrackingCallback::new(default_pricing()))
}

/// Create a CostTrackingCallback with custom pricing.
#[allow(dead_code)]
pub fn create_tracker_with_pricing(
    pricing: HashMap<String, ModelPricing>,
) -> Arc<CostTrackingCallback> {
    Arc::new(CostTrackingCallback::new(pricing))
}

/// Display usage statistics to the terminal.
pub fn display_usage(snapshot: &UsageSnapshot) {
    println!("{}", "─── Usage Report ───".bold());
    println!(
        "  {} ~{} tokens",
        "Input:".bold(),
        snapshot.total_input_tokens
    );
    println!(
        "  {} ~{} tokens",
        "Output:".bold(),
        snapshot.total_output_tokens
    );
    println!(
        "  {} ~{} tokens",
        "Total:".bold(),
        snapshot.total_input_tokens + snapshot.total_output_tokens
    );
    println!("  {} {}", "Requests:".bold(), snapshot.total_requests);

    if snapshot.estimated_cost_usd > 0.0 {
        println!(
            "  {} ${:.6}",
            "Est. Cost:".bold(),
            snapshot.estimated_cost_usd
        );
    }

    if !snapshot.per_model.is_empty() {
        println!();
        println!("  {}", "Per-model breakdown:".dimmed());
        for (model, usage) in &snapshot.per_model {
            println!(
                "    {} — {} in / {} out ({} reqs, ${:.6})",
                model.cyan(),
                usage.input_tokens,
                usage.output_tokens,
                usage.requests,
                usage.cost_usd
            );
        }
    }
}

//! Usage tracking and cost display.
//!
//! Phase 1: CostTrackingCallback wrappers (existing).
//! Phase 2: Multi-dimensional UsageTracker with per-record aggregation.
//! Phase 3: JSONL persistence to ~/.synapse/usage/records.jsonl.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use colored::Colorize;
use serde::{Deserialize, Serialize};
use synaptic::callbacks::{default_pricing, CostTrackingCallback, ModelPricing, UsageSnapshot};
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Phase 1: Framework tracker helpers (unchanged)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Phase 2: Multi-dimensional usage record & tracker
// ---------------------------------------------------------------------------

/// A single usage record with full dimensional metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub model: String,
    pub provider: String,
    pub channel: String,
    pub agent_id: String,
    pub session_key: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub latency_ms: u64,
    pub timestamp_ms: u64,
}

/// Aggregated totals across all records.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub request_count: u64,
}

/// Usage aggregated by a single dimension key.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DimensionUsage {
    pub key: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost: f64,
    pub count: u64,
}

/// Daily usage bucket.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailyUsage {
    pub date: String, // "2026-03-15"
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
    pub count: u64,
}

/// Latency distribution stats.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    pub count: u64,
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub min_ms: u64,
    pub max_ms: u64,
}

/// Full aggregated snapshot across all dimensions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregatedSnapshot {
    pub totals: UsageTotals,
    pub by_model: Vec<DimensionUsage>,
    pub by_provider: Vec<DimensionUsage>,
    pub by_channel: Vec<DimensionUsage>,
    pub by_agent: Vec<DimensionUsage>,
    pub daily: Vec<DailyUsage>,
    pub latency: LatencyStats,
}

/// Multi-dimensional usage tracker.
///
/// Wraps the framework's `CostTrackingCallback` (used by deep-agent middleware)
/// and adds dimensional record tracking with persistence.
pub struct UsageTracker {
    /// The underlying framework tracker — kept alive so the deep agent's
    /// CostTrackingCallback middleware can record aggregate snapshots.
    /// Direct reads happen via CostTrackingSubscriber, not through this field.
    #[allow(dead_code)]
    pub framework_tracker: Arc<CostTrackingCallback>,
    /// All dimensional records (in-memory).
    records: RwLock<Vec<UsageRecord>>,
    /// Index of the first record not yet flushed to disk.
    flush_cursor: RwLock<usize>,
    /// Persistence file path (e.g. ~/.synapse/usage/records.jsonl).
    persist_path: Option<PathBuf>,
}

impl UsageTracker {
    /// Create a new tracker backed by the given framework callback.
    #[allow(dead_code)]
    pub fn new(framework_tracker: Arc<CostTrackingCallback>) -> Self {
        Self {
            framework_tracker,
            records: RwLock::new(Vec::new()),
            flush_cursor: RwLock::new(0),
            persist_path: None,
        }
    }

    /// Create a new tracker with JSONL persistence.
    pub fn with_persistence(framework_tracker: Arc<CostTrackingCallback>, path: PathBuf) -> Self {
        Self {
            framework_tracker,
            records: RwLock::new(Vec::new()),
            flush_cursor: RwLock::new(0),
            persist_path: Some(path),
        }
    }

    /// Record a usage event with full dimensions.
    pub async fn record(&self, record: UsageRecord) {
        let mut records = self.records.write().await;
        records.push(record);
    }

    /// Get aggregated snapshot across all dimensions.
    #[allow(dead_code)]
    pub async fn snapshot(&self) -> AggregatedSnapshot {
        let records = self.records.read().await;
        Self::aggregate(&records)
    }

    /// Get records filtered by time range (since_ms inclusive).
    pub async fn records_since(&self, since_ms: u64) -> Vec<UsageRecord> {
        let records = self.records.read().await;
        records
            .iter()
            .filter(|r| r.timestamp_ms >= since_ms)
            .cloned()
            .collect()
    }

    /// Get aggregated snapshot filtered by time range.
    pub async fn snapshot_since(&self, since_ms: u64) -> AggregatedSnapshot {
        let records = self.records_since(since_ms).await;
        Self::aggregate(&records)
    }

    // -----------------------------------------------------------------------
    // Phase 3: Persistence
    // -----------------------------------------------------------------------

    /// Load records from the JSONL persistence file.
    pub async fn load_from_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(());
        }
        let content = tokio::fs::read_to_string(path).await?;
        let mut records = self.records.write().await;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<UsageRecord>(line) {
                Ok(record) => records.push(record),
                Err(e) => {
                    tracing::warn!(error = %e, "skipping malformed usage record line");
                }
            }
        }
        // Mark all loaded records as already flushed
        let len = records.len();
        drop(records);
        *self.flush_cursor.write().await = len;
        Ok(())
    }

    /// Save only NEW (unflushed) records to the JSONL persistence file (append mode).
    pub async fn save_to_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let records = self.records.read().await;
        let cursor = *self.flush_cursor.read().await;
        let new_records: Vec<&UsageRecord> = records.iter().skip(cursor).collect();
        if new_records.is_empty() {
            return Ok(());
        }
        // Ensure parent dir exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        // Append new records
        let mut lines = String::new();
        for record in &new_records {
            lines.push_str(&serde_json::to_string(record)?);
            lines.push('\n');
        }
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        file.write_all(lines.as_bytes()).await?;
        file.flush().await?;
        drop(records);
        // Advance cursor
        let total = self.records.read().await.len();
        *self.flush_cursor.write().await = total;
        Ok(())
    }

    /// Load records from the configured persistence file (if any).
    pub async fn load(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref path) = self.persist_path {
            self.load_from_file(path).await?;
            let count = self.records.read().await.len();
            tracing::info!(
                path = %path.display(),
                records = count,
                "loaded usage records"
            );
        }
        Ok(())
    }

    /// Flush new records to the configured persistence file (if any).
    pub async fn flush(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref path) = self.persist_path {
            self.save_to_file(path).await?;
        }
        Ok(())
    }

    /// Spawn a background task that flushes new records to disk every `interval`.
    pub fn spawn_periodic_flush(self: &Arc<Self>, interval: std::time::Duration) {
        if self.persist_path.is_none() {
            return;
        }
        let tracker = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tick.tick().await;
                if let Err(e) = tracker.flush().await {
                    tracing::warn!(error = %e, "periodic usage flush failed");
                }
            }
        });
    }

    // -----------------------------------------------------------------------
    // Aggregation helpers
    // -----------------------------------------------------------------------

    fn aggregate(records: &[UsageRecord]) -> AggregatedSnapshot {
        if records.is_empty() {
            return AggregatedSnapshot::default();
        }

        let mut totals = UsageTotals::default();
        let mut by_model: HashMap<String, DimensionUsage> = HashMap::new();
        let mut by_provider: HashMap<String, DimensionUsage> = HashMap::new();
        let mut by_channel: HashMap<String, DimensionUsage> = HashMap::new();
        let mut by_agent: HashMap<String, DimensionUsage> = HashMap::new();
        let mut daily: HashMap<String, DailyUsage> = HashMap::new();
        let mut latencies: Vec<u64> = Vec::with_capacity(records.len());

        for r in records {
            totals.input_tokens += r.input_tokens;
            totals.output_tokens += r.output_tokens;
            totals.total_tokens += r.total_tokens;
            totals.total_cost += r.cost_usd;
            totals.request_count += 1;

            // Per-dimension accumulation
            Self::accum_dimension(&mut by_model, &r.model, r);
            Self::accum_dimension(&mut by_provider, &r.provider, r);
            Self::accum_dimension(&mut by_channel, &r.channel, r);
            Self::accum_dimension(&mut by_agent, &r.agent_id, r);

            // Daily bucket (derive date from timestamp_ms)
            let date = Self::date_from_ms(r.timestamp_ms);
            let day = daily.entry(date.clone()).or_insert_with(|| DailyUsage {
                date,
                ..Default::default()
            });
            day.input_tokens += r.input_tokens;
            day.output_tokens += r.output_tokens;
            day.cost += r.cost_usd;
            day.count += 1;

            latencies.push(r.latency_ms);
        }

        // Latency stats
        latencies.sort_unstable();
        let latency = if latencies.is_empty() {
            LatencyStats::default()
        } else {
            let count = latencies.len() as u64;
            let sum: u64 = latencies.iter().sum();
            let avg_ms = sum as f64 / latencies.len() as f64;
            let p95_idx = ((latencies.len() as f64) * 0.95).ceil() as usize;
            let p95_ms = latencies[p95_idx.min(latencies.len() - 1)] as f64;
            let min_ms = *latencies.first().unwrap();
            let max_ms = *latencies.last().unwrap();
            LatencyStats {
                count,
                avg_ms,
                p95_ms,
                min_ms,
                max_ms,
            }
        };

        // Convert maps to sorted vecs
        let mut by_model: Vec<_> = by_model.into_values().collect();
        by_model.sort_by(|a, b| {
            b.cost
                .partial_cmp(&a.cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut by_provider: Vec<_> = by_provider.into_values().collect();
        by_provider.sort_by(|a, b| {
            b.cost
                .partial_cmp(&a.cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut by_channel: Vec<_> = by_channel.into_values().collect();
        by_channel.sort_by(|a, b| {
            b.cost
                .partial_cmp(&a.cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut by_agent: Vec<_> = by_agent.into_values().collect();
        by_agent.sort_by(|a, b| {
            b.cost
                .partial_cmp(&a.cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut daily: Vec<_> = daily.into_values().collect();
        daily.sort_by(|a, b| a.date.cmp(&b.date));

        AggregatedSnapshot {
            totals,
            by_model,
            by_provider,
            by_channel,
            by_agent,
            daily,
            latency,
        }
    }

    fn accum_dimension(map: &mut HashMap<String, DimensionUsage>, key: &str, r: &UsageRecord) {
        let entry = map
            .entry(key.to_string())
            .or_insert_with(|| DimensionUsage {
                key: key.to_string(),
                ..Default::default()
            });
        entry.input_tokens += r.input_tokens;
        entry.output_tokens += r.output_tokens;
        entry.total_tokens += r.total_tokens;
        entry.cost += r.cost_usd;
        entry.count += 1;
    }

    fn date_from_ms(timestamp_ms: u64) -> String {
        use std::time::{Duration, UNIX_EPOCH};
        let dt = UNIX_EPOCH + Duration::from_millis(timestamp_ms);
        // Format as YYYY-MM-DD using chrono-free approach
        let secs = dt.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        // Days since epoch
        let days = secs / 86400;
        // Simple Gregorian calendar calculation
        let (y, m, d) = Self::days_to_ymd(days);
        format!("{:04}-{:02}-{:02}", y, m, d)
    }

    fn days_to_ymd(days: u64) -> (i64, u64, u64) {
        // Algorithm from http://howardhinnant.github.io/date_algorithms.html
        let z = days as i64 + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = (z - era * 146097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        (y, m, d)
    }
}

/// Default persistence path: ~/.synapse/usage/records.jsonl
pub fn default_usage_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".synapse")
        .join("usage")
        .join("records.jsonl")
}

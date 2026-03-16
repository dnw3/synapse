use crate::config::reset_policy::{ResetMode, ResetPolicy};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionFreshness {
    pub fresh: bool,
    pub daily_reset_at: Option<u64>,
    pub idle_expires_at: Option<u64>,
    pub reason: Option<String>,
}

/// Evaluate whether a session is fresh or needs reset.
#[allow(dead_code)]
pub fn evaluate_freshness(updated_at: u64, policy: &ResetPolicy, now_ms: u64) -> SessionFreshness {
    match policy.mode {
        ResetMode::Never => SessionFreshness {
            fresh: true,
            daily_reset_at: None,
            idle_expires_at: None,
            reason: None,
        },
        ResetMode::Daily => {
            // Compute today's reset boundary
            let now_secs = now_ms / 1000;
            let secs_in_day = 86400u64;
            let today_start = (now_secs / secs_in_day) * secs_in_day; // midnight UTC
            let reset_boundary_secs = today_start + (policy.at_hour as u64) * 3600;
            let reset_boundary_ms = reset_boundary_secs * 1000;

            // If now is before today's reset hour, use yesterday's boundary
            let effective_boundary = if now_ms < reset_boundary_ms {
                reset_boundary_ms - secs_in_day * 1000
            } else {
                reset_boundary_ms
            };

            let fresh = updated_at >= effective_boundary;
            SessionFreshness {
                fresh,
                daily_reset_at: Some(effective_boundary),
                idle_expires_at: None,
                reason: if !fresh {
                    Some("daily reset boundary crossed".into())
                } else {
                    None
                },
            }
        }
        ResetMode::Idle => {
            let idle_ms = policy.idle_minutes as u64 * 60 * 1000;
            let expires_at = updated_at + idle_ms;
            let fresh = now_ms < expires_at;
            SessionFreshness {
                fresh,
                daily_reset_at: None,
                idle_expires_at: Some(expires_at),
                reason: if !fresh {
                    Some(format!("idle for {}+ minutes", policy.idle_minutes))
                } else {
                    None
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::reset_policy::{ResetMode, ResetPolicy};

    fn daily_policy(at_hour: u8) -> ResetPolicy {
        ResetPolicy {
            mode: ResetMode::Daily,
            at_hour,
            idle_minutes: 60,
        }
    }

    fn idle_policy(idle_minutes: u32) -> ResetPolicy {
        ResetPolicy {
            mode: ResetMode::Idle,
            at_hour: 4,
            idle_minutes,
        }
    }

    fn never_policy() -> ResetPolicy {
        ResetPolicy {
            mode: ResetMode::Never,
            at_hour: 4,
            idle_minutes: 60,
        }
    }

    /// 2024-01-15 12:00:00 UTC in milliseconds
    const NOW_MS: u64 = 1705316400_000;

    #[test]
    fn test_fresh_session_updated_recently() {
        // Updated 5 minutes ago — well within any window
        let updated_at = NOW_MS - 5 * 60 * 1000;
        let result = evaluate_freshness(updated_at, &idle_policy(60), NOW_MS);
        assert!(result.fresh);
        assert!(result.reason.is_none());
        assert!(result.idle_expires_at.is_some());
    }

    #[test]
    fn test_stale_daily_updated_before_reset_hour() {
        // Reset at 04:00 UTC. NOW_MS is 12:00 UTC on 2024-01-15.
        // effective_boundary = 2024-01-15 04:00 UTC
        // Updated at 2024-01-15 03:00 UTC — before the boundary → stale
        let boundary_ms = {
            let now_secs = NOW_MS / 1000;
            let today_start = (now_secs / 86400) * 86400;
            (today_start + 4 * 3600) * 1000
        };
        let updated_at = boundary_ms - 3600 * 1000; // 1 hour before boundary
        let result = evaluate_freshness(updated_at, &daily_policy(4), NOW_MS);
        assert!(!result.fresh);
        assert!(result.reason.is_some());
        assert_eq!(result.daily_reset_at, Some(boundary_ms));
    }

    #[test]
    fn test_fresh_daily_updated_after_reset_hour() {
        // Updated at 05:00 UTC today — after the 04:00 reset boundary → fresh
        let boundary_ms = {
            let now_secs = NOW_MS / 1000;
            let today_start = (now_secs / 86400) * 86400;
            (today_start + 4 * 3600) * 1000
        };
        let updated_at = boundary_ms + 3600 * 1000; // 1 hour after boundary
        let result = evaluate_freshness(updated_at, &daily_policy(4), NOW_MS);
        assert!(result.fresh);
        assert!(result.reason.is_none());
    }

    #[test]
    fn test_stale_idle_updated_more_than_idle_minutes_ago() {
        // idle_minutes = 30. Updated 45 minutes ago → stale
        let updated_at = NOW_MS - 45 * 60 * 1000;
        let result = evaluate_freshness(updated_at, &idle_policy(30), NOW_MS);
        assert!(!result.fresh);
        assert!(result.reason.as_deref().unwrap().contains("30"));
        let expires_at = result.idle_expires_at.unwrap();
        assert!(expires_at < NOW_MS);
    }

    #[test]
    fn test_never_mode_always_fresh() {
        // Updated a year ago — never mode always returns fresh
        let updated_at = NOW_MS - 365 * 24 * 3600 * 1000;
        let result = evaluate_freshness(updated_at, &never_policy(), NOW_MS);
        assert!(result.fresh);
        assert!(result.reason.is_none());
        assert!(result.daily_reset_at.is_none());
        assert!(result.idle_expires_at.is_none());
    }

    #[test]
    fn test_daily_before_reset_hour_uses_yesterday_boundary() {
        // NOW is 2024-01-15 02:00 UTC — before the 04:00 reset hour.
        // effective_boundary should be yesterday's 04:00, i.e. 2024-01-14 04:00 UTC.
        let now_before_reset = {
            let now_secs = NOW_MS / 1000;
            let today_start = (now_secs / 86400) * 86400;
            (today_start + 2 * 3600) * 1000 // 02:00 UTC
        };
        let yesterday_boundary = {
            let now_secs = now_before_reset / 1000;
            let today_start = (now_secs / 86400) * 86400;
            // today's 04:00 is in the future, so we go back one day
            (today_start + 4 * 3600 - 86400) * 1000
        };

        // Updated at yesterday's 05:00 UTC — after yesterday's boundary → fresh
        let updated_after_yesterday_boundary = yesterday_boundary + 3600 * 1000;
        let result =
            evaluate_freshness(updated_after_yesterday_boundary, &daily_policy(4), now_before_reset);
        assert!(result.fresh);
        assert_eq!(result.daily_reset_at, Some(yesterday_boundary));

        // Updated at yesterday's 03:00 UTC — before yesterday's boundary → stale
        let updated_before_yesterday_boundary = yesterday_boundary - 3600 * 1000;
        let result = evaluate_freshness(
            updated_before_yesterday_boundary,
            &daily_policy(4),
            now_before_reset,
        );
        assert!(!result.fresh);
    }

    #[test]
    fn test_idle_exact_boundary_is_stale() {
        // now_ms == expires_at: the condition is `now_ms < expires_at`, so exact boundary is stale
        let idle_ms = 60u64 * 60 * 1000;
        let updated_at = NOW_MS - idle_ms;
        let result = evaluate_freshness(updated_at, &idle_policy(60), NOW_MS);
        assert!(!result.fresh);
        assert_eq!(result.idle_expires_at, Some(NOW_MS));
    }

    #[test]
    fn test_daily_updated_exactly_at_boundary_is_fresh() {
        // updated_at == effective_boundary: condition is `>=`, so exactly at boundary is fresh
        let boundary_ms = {
            let now_secs = NOW_MS / 1000;
            let today_start = (now_secs / 86400) * 86400;
            (today_start + 4 * 3600) * 1000
        };
        let result = evaluate_freshness(boundary_ms, &daily_policy(4), NOW_MS);
        assert!(result.fresh);
    }
}

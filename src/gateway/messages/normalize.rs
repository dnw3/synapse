use super::outbound::OutboundPayload;

/// Phase 1: Normalize payloads for delivery.
#[allow(dead_code)]
/// Filters reasoning, merges consecutive text, validates renderability.
pub fn normalize_for_delivery(
    payloads: Vec<OutboundPayload>,
    show_reasoning: bool,
) -> Vec<OutboundPayload> {
    let mut result: Vec<OutboundPayload> = Vec::new();

    for payload in payloads {
        // Skip reasoning unless display is enabled
        if payload.is_reasoning && !show_reasoning {
            continue;
        }

        // Skip empty payloads (no text and no media)
        if payload.text.as_deref().is_none_or(|t| t.is_empty())
            && payload.media_urls.is_empty()
            && payload.interactive.is_none()
        {
            continue;
        }

        // Merge consecutive text-only payloads
        if let Some(last) = result.last_mut() {
            if last.media_urls.is_empty()
                && last.interactive.is_none()
                && payload.media_urls.is_empty()
                && payload.interactive.is_none()
                && !payload.is_error
            {
                // Merge text
                if let Some(ref mut last_text) = last.text {
                    if let Some(ref new_text) = payload.text {
                        last_text.push('\n');
                        last_text.push_str(new_text);
                        continue;
                    }
                }
            }
        }

        result.push(payload);
    }

    result
}

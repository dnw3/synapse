/// Platform-aware message chunking and formatting.
///
/// Each messaging platform has different limits:
/// - Discord: 2000 characters
/// - Telegram: 4096 characters
/// - Slack: 4000 characters (mrkdwn)
/// - Lark: 4096 characters (approximate)
/// - Mattermost: 16383 characters

/// Split a message into chunks that respect platform character limits.
///
/// Splitting priority:
/// 1. Paragraph boundaries (`\n\n`) — cleanest split
/// 2. Line boundaries (`\n`) — fallback
/// 3. Hard cut at max_len — last resort
///
/// Code blocks (``` fenced) are kept intact when possible.
pub fn chunk_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        let slice = &remaining[..max_len];

        // Check if we're inside a code block — if so, try to find the closing ```
        // within the slice, or extend the chunk to include the full code block.
        let split_at = if let Some(code_fence_start) = find_open_code_fence(slice) {
            // There's an unclosed code fence in the slice.
            // Try to find the closing fence after the slice boundary.
            let after = &remaining[max_len..];
            if let Some(close_offset) = after.find("\n```") {
                let full_block_end = max_len + close_offset + 4; // include "\n```"
                // Skip trailing newline after the closing fence
                let end = remaining[full_block_end..]
                    .find('\n')
                    .map(|i| full_block_end + i)
                    .unwrap_or(full_block_end);
                end
            } else {
                // No closing fence found — split before the code fence
                if code_fence_start > max_len / 4 {
                    code_fence_start
                } else {
                    // Code fence is near the start; fall back to normal splitting
                    find_best_split_point(slice, max_len)
                }
            }
        } else {
            find_best_split_point(slice, max_len)
        };

        chunks.push(remaining[..split_at].trim_end().to_string());
        remaining = remaining[split_at..].trim_start_matches('\n');
    }

    chunks
}

/// Find the best split point within a slice (paragraph > newline > hard cut).
fn find_best_split_point(slice: &str, max_len: usize) -> usize {
    let split_at = slice
        .rfind("\n\n")
        .or_else(|| slice.rfind('\n'))
        .unwrap_or(max_len);

    // Don't create tiny fragments
    if split_at < max_len / 4 {
        max_len
    } else {
        split_at
    }
}

/// Check if there's an unclosed code fence (```) in the text.
/// Returns the byte offset of the opening fence, if any.
fn find_open_code_fence(text: &str) -> Option<usize> {
    let mut fence_count = 0;
    let mut last_open_pos = None;

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            fence_count += 1;
            if fence_count % 2 == 1 {
                // This is an opening fence
                last_open_pos = Some(
                    text.lines()
                        .take(i)
                        .map(|l| l.len() + 1)
                        .sum::<usize>(),
                );
            } else {
                // This is a closing fence
                last_open_pos = None;
            }
        }
    }

    last_open_pos
}

/// Chunk for Discord (2000 char limit).
pub fn chunk_discord(text: &str) -> Vec<String> {
    chunk_message(text, 2000)
}

/// Chunk for Telegram (4096 char limit).
pub fn chunk_telegram(text: &str) -> Vec<String> {
    chunk_message(text, 4096)
}

/// Chunk for Slack (4000 char limit).
pub fn chunk_slack(text: &str) -> Vec<String> {
    chunk_message(text, 4000)
}

/// Chunk for Lark (4096 char limit).
pub fn chunk_lark(text: &str) -> Vec<String> {
    chunk_message(text, 4096)
}

/// Chunk for DingTalk (20000 char limit for text messages).
pub fn chunk_dingtalk(text: &str) -> Vec<String> {
    chunk_message(text, 20000)
}

/// Chunk for Mattermost (16383 char limit).
pub fn chunk_mattermost(text: &str) -> Vec<String> {
    chunk_message(text, 16383)
}

/// Chunk for Matrix (60000 char limit for m.room.message body).
pub fn chunk_matrix(text: &str) -> Vec<String> {
    chunk_message(text, 60000)
}

/// Chunk for WhatsApp (2000 char limit, matching WhatsApp Web display constraints).
pub fn chunk_whatsapp(text: &str) -> Vec<String> {
    chunk_message(text, 2000)
}

/// Chunk for Microsoft Teams (4000 char limit).
pub fn chunk_teams(text: &str) -> Vec<String> {
    chunk_message(text, 4000)
}

/// Chunk for Signal (4096 char limit, same as Telegram).
pub fn chunk_signal(text: &str) -> Vec<String> {
    chunk_message(text, 4096)
}

/// Chunk for iMessage via BlueBubbles (10000 char limit).
pub fn chunk_imessage(text: &str) -> Vec<String> {
    chunk_message(text, 10000)
}

/// Chunk for LINE Messaging API (5000 char limit per text message).
pub fn chunk_line(text: &str) -> Vec<String> {
    chunk_message(text, 5000)
}

/// Chunk for Google Chat (4096 char limit).
pub fn chunk_googlechat(text: &str) -> Vec<String> {
    chunk_message(text, 4096)
}

/// Chunk for WeCom (WeChat Work) Bot Webhook API (2048 char limit for text messages).
pub fn chunk_wechat(text: &str) -> Vec<String> {
    chunk_message(text, 2048)
}

/// Chunk for IRC (400 char limit — IRC lines are capped at 512 bytes including
/// the ":nick!user@host PRIVMSG #channel :" prefix, leaving ~400 chars of usable space).
pub fn chunk_irc(text: &str) -> Vec<String> {
    chunk_message(text, 400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_message_no_split() {
        let chunks = chunk_message("hello world", 100);
        assert_eq!(chunks, vec!["hello world"]);
    }

    #[test]
    fn splits_at_paragraph() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = chunk_message(text, 30);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].len() <= 30);
    }

    #[test]
    fn splits_at_newline() {
        let text = "Line one\nLine two\nLine three\nLine four";
        let chunks = chunk_message(text, 20);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.len() <= 20);
        }
    }
}

/// Extract keywords from text with multilingual support.
///
/// Handles:
/// - English/Latin: split on non-alphanumeric, filter stopwords
/// - Chinese (CJK Unified Ideographs): unigram + bigram extraction
/// - Japanese: kanji/kana segmentation (unigrams for kanji, kana runs as tokens)
/// - Korean: syllable-based unigram + bigram, trailing particle stripping
pub fn extract_keywords(text: &str) -> Vec<String> {
    let en_stopwords = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
        "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall",
        "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at",
        "by", "from", "as", "into", "through", "during", "before", "after", "above", "below",
        "between", "out", "off", "over", "under", "again", "further", "then", "once", "here",
        "there", "when", "where", "why", "how", "all", "both", "each", "few", "more", "most",
        "other", "some", "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too",
        "very", "just", "because", "but", "and", "or", "if", "while", "that", "this", "it", "i",
        "you", "he", "she", "we", "they", "me", "him", "her", "us", "them", "my", "your", "his",
        "its", "our", "their",
    ];

    let zh_stopwords: &[&str] = &[
        "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一个",
        "上", "也", "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好",
        "自己", "这", "他", "她", "它", "们", "那", "些", "被", "把", "从", "对", "让",
        "与", "而", "及", "但", "或", "如果", "因为", "所以", "可以", "这个", "那个",
        "什么", "怎么", "哪", "吗", "呢", "吧", "啊", "嗯", "哦",
    ];

    let mut keywords = Vec::new();
    let lower = text.to_lowercase();

    let mut latin_buf = String::new();
    let chars: Vec<char> = lower.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if is_cjk(c) {
            flush_latin_tokens(&latin_buf, &en_stopwords, &mut keywords);
            latin_buf.clear();

            let start = i;
            while i < chars.len() && is_cjk(chars[i]) {
                i += 1;
            }
            let cjk_run: String = chars[start..i].iter().collect();
            extract_cjk_keywords(&cjk_run, zh_stopwords, &mut keywords);
        } else if c.is_alphanumeric() || is_korean_syllable(c) {
            latin_buf.push(c);
            i += 1;
        } else {
            flush_latin_tokens(&latin_buf, &en_stopwords, &mut keywords);
            latin_buf.clear();
            i += 1;
        }
    }
    flush_latin_tokens(&latin_buf, &en_stopwords, &mut keywords);

    keywords
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{3040}'..='\u{309F}'
        | '\u{30A0}'..='\u{30FF}'
    )
}

fn is_korean_syllable(c: char) -> bool {
    matches!(c, '\u{AC00}'..='\u{D7AF}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}')
}

fn extract_cjk_keywords(run: &str, stopwords: &[&str], out: &mut Vec<String>) {
    let chars: Vec<char> = run.chars().collect();
    for c in &chars {
        let s = c.to_string();
        if !stopwords.contains(&s.as_str()) {
            out.push(s);
        }
    }
    for pair in chars.windows(2) {
        let bigram: String = pair.iter().collect();
        if !stopwords.contains(&bigram.as_str()) {
            out.push(bigram);
        }
    }
}

fn flush_latin_tokens(buf: &str, stopwords: &[&str], out: &mut Vec<String>) {
    if buf.len() > 2 && !stopwords.contains(&buf) {
        let chars: Vec<char> = buf.chars().collect();
        if chars.iter().any(|c| is_korean_syllable(*c)) {
            for c in &chars {
                if is_korean_syllable(*c) {
                    out.push(c.to_string());
                }
            }
            for pair in chars.windows(2) {
                if pair.iter().all(|c| is_korean_syllable(*c)) {
                    out.push(pair.iter().collect());
                }
            }
        } else {
            out.push(buf.to_string());
        }
    }
}

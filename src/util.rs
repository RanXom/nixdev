use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use unicode_width::UnicodeWidthStr;

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn format_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn sanitize_project_name(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches(|c| c == '-' || c == '.' || c == '_');
    let clipped: String = trimmed.chars().take(64).collect();
    if clipped.is_empty() {
        "project".to_string()
    } else {
        clipped
    }
}

pub fn project_name_from_dir(dir: &Path) -> String {
    let name = dir
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".to_string());
    sanitize_project_name(&name)
}

pub fn valid_attr(attr: &str) -> bool {
    if attr.is_empty() || attr.len() > 160 {
        return false;
    }
    attr.split('.').all(|segment| {
        let mut chars = segment.chars();
        match chars.next() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
            _ => return false,
        }
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '\'')
    })
}

pub fn valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn fuzzy_score(haystack: &str, needle: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let hay: Vec<char> = haystack.to_lowercase().chars().collect();
    let ndl: Vec<char> = needle.to_lowercase().chars().collect();
    let mut score = 0;
    let mut hi = 0;
    let mut last_match: Option<usize> = None;
    for ch in &ndl {
        let mut found = None;
        while hi < hay.len() {
            if hay[hi] == *ch {
                found = Some(hi);
                break;
            }
            hi += 1;
        }
        let idx = found?;
        score += 10;
        if let Some(prev) = last_match {
            if idx == prev + 1 {
                score += 15;
            }
        }
        if idx == 0 || hay[idx - 1] == '-' || hay[idx - 1] == '_' || hay[idx - 1] == '.' {
            score += 20;
        }
        last_match = Some(idx);
        hi = idx + 1;
    }
    Some(score - hay.len() as i32 / 8)
}

pub fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text.width() <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = ch.to_string().width();
        if used + w > width.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

pub fn ellipsize_middle(text: &str, width: usize) -> String {
    if text.width() <= width || width < 8 {
        return truncate(text, width);
    }
    let keep = width - 1;
    let front = keep / 2;
    let back = keep - front;
    let chars: Vec<char> = text.chars().collect();
    let start: String = chars.iter().take(front).collect();
    let end: String = chars.iter().skip(chars.len() - back).collect();
    format!("{start}…{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        assert_eq!(format_date(0), "1970-01-01");
        assert_eq!(format_date(1_700_000_000), "2023-11-14");
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_project_name("my cool project"), "my-cool-project");
        assert_eq!(sanitize_project_name("---"), "project");
        assert_eq!(sanitize_project_name("a/b"), "a-b");
    }

    #[test]
    fn rejects_dangerous_attrs() {
        assert!(valid_attr("python3Packages.requests"));
        assert!(valid_attr("rust-analyzer"));
        assert!(!valid_attr("foo; rm -rf /"));
        assert!(!valid_attr(""));
        assert!(!valid_attr(".hidden"));
    }

    #[test]
    fn fuzzy_matches_subsequences() {
        assert!(fuzzy_score("rust-analyzer", "rsa").is_some());
        assert!(fuzzy_score("rust-analyzer", "zzz").is_none());
        assert!(fuzzy_score("rust-analyzer", "rust") > fuzzy_score("rust-analyzer", "rzt"));
    }
}

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    pub timestamp: Duration,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedLyrics {
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsWindow<'a> {
    pub prev: Option<&'a LyricLine>,
    pub curr: Option<&'a LyricLine>,
    pub next: Option<&'a LyricLine>,
    pub is_instrumental_gap: bool,
}

impl SyncedLyrics {
    pub fn from_lrc(content: &str) -> Option<Self> {
        let mut lines = Vec::new();
        let mut offset_ms: i64 = 0;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            // Check for metadata tag [offset:+/-ms]
            if line.starts_with("[offset:") && line.ends_with(']') {
                let val_str = &line[8..line.len() - 1].trim();
                if let Ok(parsed) = val_str.parse::<i64>() {
                    offset_ms = parsed;
                }
                continue;
            }

            // Skip other metadata tags like [ti:], [ar:], [al:], etc.
            if Self::is_metadata_tag(line) {
                continue;
            }

            // Extract all timestamps from line: [mm:ss.xx][mm:ss.xx] Text
            let mut timestamps = Vec::new();
            let mut text_start_idx = 0;

            let bytes = line.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'[' {
                    if let Some(close_pos) = line[i..].find(']') {
                        let tag = &line[i + 1..i + close_pos];
                        if let Some(dur) = Self::parse_timestamp(tag) {
                            timestamps.push(dur);
                            i += close_pos + 1;
                            text_start_idx = i;
                            continue;
                        }
                    }
                }
                break;
            }

            if timestamps.is_empty() {
                continue;
            }

            let lyric_text = line[text_start_idx..].trim().to_string();

            for ts in timestamps {
                let adjusted_ms = if offset_ms != 0 {
                    let raw_ms = ts.as_millis() as i64 + offset_ms;
                    if raw_ms > 0 {
                        raw_ms as u64
                    } else {
                        0
                    }
                } else {
                    ts.as_millis() as u64
                };

                lines.push(LyricLine {
                    timestamp: Duration::from_millis(adjusted_ms),
                    text: lyric_text.clone(),
                });
            }
        }

        if lines.is_empty() {
            return None;
        }

        // Sort by timestamp
        lines.sort_by_key(|l| l.timestamp);

        Some(Self { lines })
    }

    fn is_metadata_tag(line: &str) -> bool {
        if !line.starts_with('[') || !line.contains(':') {
            return false;
        }
        let Some(colon_pos) = line.find(':') else {
            return false;
        };
        let tag_name = &line[1..colon_pos].trim().to_lowercase();
        matches!(
            tag_name.as_str(),
            "ti" | "ar" | "al" | "by" | "length" | "author" | "creator" | "re" | "ve"
        )
    }

    fn parse_timestamp(tag: &str) -> Option<Duration> {
        // Formats: mm:ss.xx, mm:ss.xxx, mm:ss
        let parts: Vec<&str> = tag.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let mins: u64 = parts[0].trim().parse().ok()?;
        let sec_parts: Vec<&str> = parts[1].split('.').collect();

        let secs: u64 = sec_parts[0].trim().parse().ok()?;
        let millis: u64 = if sec_parts.len() == 2 {
            let frac_str = sec_parts[1].trim();
            match frac_str.len() {
                1 => frac_str.parse::<u64>().ok()? * 100,
                2 => frac_str.parse::<u64>().ok()? * 10,
                3 => frac_str.parse::<u64>().ok()?,
                _ => frac_str[..3].parse::<u64>().ok()?,
            }
        } else {
            0
        };

        let total_ms = mins * 60_000 + secs * 1_000 + millis;
        Some(Duration::from_millis(total_ms))
    }

    /// Calculate (previous, current, next) lines relative to elapsed playback position.
    pub fn get_window(&self, elapsed: Duration) -> LyricsWindow<'_> {
        if self.lines.is_empty() {
            return LyricsWindow {
                prev: None,
                curr: None,
                next: None,
                is_instrumental_gap: false,
            };
        }

        // Before first lyric
        if elapsed < self.lines[0].timestamp {
            return LyricsWindow {
                prev: None,
                curr: None,
                next: self.lines.first(),
                is_instrumental_gap: true,
            };
        }

        // Find current line index (last line where timestamp <= elapsed)
        let curr_idx = self.lines.iter().rposition(|l| l.timestamp <= elapsed).unwrap();
        let curr_line = &self.lines[curr_idx];

        let prev_line = if curr_idx > 0 {
            // Find most recent previous non-empty lyric line if possible, or immediate previous
            Some(&self.lines[curr_idx - 1])
        } else {
            None
        };

        let next_line = self.lines.get(curr_idx + 1);

        // Check if we are currently in an instrumental break
        let is_instrumental_gap = if curr_line.text.trim().is_empty() {
            true
        } else if let Some(next) = next_line {
            // If current line finished playing (e.g. > 9s elapsed since timestamp)
            // and next line is still more than 4s away
            let time_since_curr = elapsed.saturating_sub(curr_line.timestamp);
            let time_until_next = next.timestamp.saturating_sub(elapsed);
            time_since_curr >= Duration::from_secs(9) && time_until_next >= Duration::from_secs(4)
        } else {
            // Past last line: if more than 10 seconds past last line, it's instrumental outro
            elapsed.saturating_sub(curr_line.timestamp) >= Duration::from_secs(10)
        };

        LyricsWindow {
            prev: prev_line,
            curr: Some(curr_line),
            next: next_line,
            is_instrumental_gap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lrc_basic() {
        let sample = "\
[ti: Creep]
[ar: Radiohead]
[al: Pablo Honey]
[00:19.16] When you were here before
[00:24.09] Couldn't look you in the eye
[00:29.24] You're just like an angel
";
        let synced = SyncedLyrics::from_lrc(sample).expect("Failed to parse basic LRC");
        assert_eq!(synced.lines.len(), 3);
        assert_eq!(synced.lines[0].timestamp, Duration::from_millis(19_160));
        assert_eq!(synced.lines[0].text, "When you were here before");
        assert_eq!(synced.lines[1].timestamp, Duration::from_millis(24_090));
        assert_eq!(synced.lines[2].timestamp, Duration::from_millis(29_240));
    }

    #[test]
    fn test_lrc_window() {
        let sample = "\
[00:10.00] Line 1
[00:20.00] Line 2
[00:45.00] Line 3
";
        let synced = SyncedLyrics::from_lrc(sample).unwrap();

        // 1. Before Line 1
        let win = synced.get_window(Duration::from_secs(5));
        assert!(win.curr.is_none());
        assert_eq!(win.next.unwrap().text, "Line 1");
        assert!(win.is_instrumental_gap);

        // 2. During Line 1
        let win = synced.get_window(Duration::from_millis(12_000));
        assert_eq!(win.curr.unwrap().text, "Line 1");
        assert_eq!(win.next.unwrap().text, "Line 2");
        assert!(!win.is_instrumental_gap);

        // 3. During Line 2 active
        let win = synced.get_window(Duration::from_millis(22_000));
        assert_eq!(win.prev.unwrap().text, "Line 1");
        assert_eq!(win.curr.unwrap().text, "Line 2");
        assert_eq!(win.next.unwrap().text, "Line 3");
        assert!(!win.is_instrumental_gap);

        // 4. During gap between Line 2 and Line 3 (at 35s: 15s after Line 2, 10s before Line 3)
        let win = synced.get_window(Duration::from_secs(35));
        assert_eq!(win.curr.unwrap().text, "Line 2");
        assert!(win.is_instrumental_gap);
    }
}

use std::time::Duration;

const TRANSLATION_TOLERANCE: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LyricLine {
    pub time: Duration,
    pub text: String,
    pub translation: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TimedText {
    time: Duration,
    text: String,
}

pub fn parse_lrc(lrc: &str, tlyric: Option<&str>) -> Vec<LyricLine> {
    let primary = parse_timed_text(lrc);
    if primary.is_empty() {
        return Vec::new();
    }

    let mut lines = primary
        .into_iter()
        .map(|line| LyricLine {
            time: line.time,
            text: line.text,
            translation: None,
        })
        .collect::<Vec<_>>();

    if let Some(tlyric) = tlyric {
        merge_translations(&mut lines, &parse_timed_text(tlyric));
    }
    lines
}

pub fn line_index_at(lines: &[LyricLine], position: Duration) -> Option<usize> {
    lines
        .partition_point(|line| line.time <= position)
        .checked_sub(1)
}

fn parse_timed_text(input: &str) -> Vec<TimedText> {
    let offset = input
        .lines()
        .filter_map(parse_offset)
        .next_back()
        .unwrap_or(0);
    let mut lines = Vec::new();
    for raw_line in input.lines() {
        parse_lyric_line(raw_line, offset, &mut lines);
    }
    lines.sort_by_key(|line| line.time);
    collapse_empty_lines(lines)
}

fn parse_lyric_line(raw_line: &str, offset: i64, output: &mut Vec<TimedText>) {
    let mut remaining = raw_line.trim();
    if let Some(without_bom) = remaining.strip_prefix('\u{feff}') {
        remaining = without_bom.trim_start();
    }

    let mut timestamps = Vec::new();
    while let Some(after_open) = remaining.strip_prefix('[') {
        let Some(close) = after_open.find(']') else {
            break;
        };
        let Some(milliseconds) = parse_timestamp(&after_open[..close]) else {
            break;
        };
        timestamps.push(apply_offset(milliseconds, offset));
        remaining = &after_open[close + 1..];
    }

    if timestamps.is_empty() {
        return;
    }
    let text = remaining.trim().to_owned();
    output.extend(timestamps.into_iter().map(|time| TimedText {
        time,
        text: text.clone(),
    }));
}

fn parse_timestamp(tag: &str) -> Option<u64> {
    let (minutes, seconds_and_fraction) = tag.split_once(':')?;
    let minutes = parse_digits(minutes)?;
    let (seconds, fraction) = match seconds_and_fraction.split_once('.') {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (seconds_and_fraction, None),
    };
    if seconds.len() != 2 {
        return None;
    }
    let seconds = parse_digits(seconds)?;
    if seconds >= 60 {
        return None;
    }

    let fraction = match fraction {
        None => 0,
        Some(value) if (1..=3).contains(&value.len()) => {
            let value = parse_digits(value)?;
            match seconds_and_fraction
                .rsplit_once('.')
                .map(|(_, part)| part.len())
            {
                Some(1) => value * 100,
                Some(2) => value * 10,
                Some(3) => value,
                _ => return None,
            }
        }
        Some(_) => return None,
    };

    minutes
        .checked_mul(60_000)?
        .checked_add(seconds.checked_mul(1_000)?)?
        .checked_add(fraction)
}

fn parse_digits(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn parse_offset(line: &str) -> Option<i64> {
    let mut line = line.trim();
    if let Some(without_bom) = line.strip_prefix('\u{feff}') {
        line = without_bom.trim_start();
    }
    let after_open = line.strip_prefix('[')?;
    let close = after_open.find(']')?;
    let (key, value) = after_open[..close].split_once(':')?;
    if !key.trim().eq_ignore_ascii_case("offset") {
        return None;
    }
    value.trim().parse().ok()
}

fn apply_offset(milliseconds: u64, offset: i64) -> Duration {
    let adjusted = if offset >= 0 {
        milliseconds.saturating_add(offset as u64)
    } else {
        milliseconds.saturating_sub(offset.unsigned_abs())
    };
    Duration::from_millis(adjusted)
}

fn collapse_empty_lines(lines: Vec<TimedText>) -> Vec<TimedText> {
    let mut collapsed = Vec::<TimedText>::with_capacity(lines.len());
    for line in lines {
        let repeats_empty = line.text.is_empty()
            && collapsed
                .last()
                .is_some_and(|previous| previous.text.is_empty());
        if !repeats_empty {
            collapsed.push(line);
        }
    }
    collapsed
}

fn merge_translations(lines: &mut [LyricLine], translations: &[TimedText]) {
    let mut used = vec![false; translations.len()];
    for line in lines {
        let mut nearest = None::<(usize, Duration)>;
        for (index, translation) in translations.iter().enumerate() {
            if used[index] {
                continue;
            }
            let distance = duration_distance(line.time, translation.time);
            if distance > TRANSLATION_TOLERANCE {
                continue;
            }
            if nearest.is_none_or(|(_, best_distance)| distance < best_distance) {
                nearest = Some((index, distance));
            }
        }
        if let Some((index, _)) = nearest {
            used[index] = true;
            line.translation = Some(translations[index].text.clone());
        }
    }
}

fn duration_distance(left: Duration, right: Duration) -> Duration {
    left.abs_diff(right)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{line_index_at, parse_lrc, LyricLine};

    #[test]
    fn parses_realistic_lines_with_stable_time_sorting() {
        let fixture = r#"
[ti:纸灯]
[ar:遠い町]
[75:02.125]  很久以后，灯还亮着
[0:03] 夜の窓に星が揺れる
[00:12.50] 风从旧站台经过
[00:12.50] 同じ時刻の返事
[al:雨夜手帖]
"#;

        let lines = parse_lrc(fixture, None);

        assert_eq!(lines.len(), 4);
        assert_line(&lines[0], 3_000, "夜の窓に星が揺れる");
        assert_line(&lines[1], 12_500, "风从旧站台经过");
        assert_line(&lines[2], 12_500, "同じ時刻の返事");
        assert_line(&lines[3], 4_502_125, "很久以后，灯还亮着");
    }

    #[test]
    fn expands_multiple_time_tags_and_fraction_widths() {
        let lines = parse_lrc("[00:01.2][00:02.34][0:03.456]  雨粒はガラスを叩く  ", None);

        assert_eq!(lines.len(), 3);
        assert_line(&lines[0], 1_200, "雨粒はガラスを叩く");
        assert_line(&lines[1], 2_340, "雨粒はガラスを叩く");
        assert_line(&lines[2], 3_456, "雨粒はガラスを叩く");
    }

    #[test]
    fn applies_positive_and_negative_offsets_with_zero_clamping() {
        let positive = parse_lrc("[00:01.00] 纸船顺流而下\n[offset:+250]", None);
        assert_line(&positive[0], 1_250, "纸船顺流而下");

        let negative = parse_lrc(
            "[offset:-1500]\n[00:01] 朝焼けを待つ\n[00:02] 云层渐亮",
            None,
        );
        assert_line(&negative[0], 0, "朝焼けを待つ");
        assert_line(&negative[1], 500, "云层渐亮");
    }

    #[test]
    fn keeps_one_line_from_each_consecutive_empty_run() {
        let lines = parse_lrc(
            "[00:01] 灯影\n[00:02]\n[00:03]   \n[00:04] 雨音が近づく\n[00:05]",
            None,
        );

        assert_eq!(lines.len(), 4);
        assert_line(&lines[0], 1_000, "灯影");
        assert_line(&lines[1], 2_000, "");
        assert_line(&lines[2], 4_000, "雨音が近づく");
        assert_line(&lines[3], 5_000, "");
    }

    #[test]
    fn merges_the_nearest_translation_within_fifty_milliseconds() {
        let lrc = "[00:10.000] 风来到窗边\n[00:20.000] 灯火沿河醒来\n[00:30.000] 清晨仍未抵达";
        let tlyric = r#"
[offset:+10]
[00:09.940] 近いが遠い候補
[00:10.000] 窓辺に風が来る
[00:19.990] 川沿いの灯りが目覚める
[00:30.041] 五十一ミリ秒遅い
[00:40.000] 余った翻訳
"#;

        let lines = parse_lrc(lrc, Some(tlyric));

        assert_eq!(lines[0].translation.as_deref(), Some("窓辺に風が来る"));
        assert_eq!(
            lines[1].translation.as_deref(),
            Some("川沿いの灯りが目覚める")
        );
        assert_eq!(lines[2].translation, None);
    }

    #[test]
    fn invalid_or_empty_input_returns_no_lines() {
        assert!(parse_lrc("", None).is_empty());
        assert!(parse_lrc("[ti:无时间]\n只是普通文本\n[99:99]错误秒数", None).is_empty());
        assert!(parse_lrc("[18446744073709551615:59.999]溢出", None).is_empty());

        let lines = parse_lrc("[00:01] 有效行", Some("[00:xx] invalid"));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].translation, None);
    }

    #[test]
    fn finds_current_line_at_binary_search_boundaries() {
        let lines = parse_lrc("[00:01] 第一行\n[00:02] 二行目\n[00:03] 最后一行", None);

        assert_eq!(line_index_at(&[], Duration::ZERO), None);
        assert_eq!(line_index_at(&lines, Duration::from_millis(999)), None);
        assert_eq!(line_index_at(&lines, Duration::from_secs(1)), Some(0));
        assert_eq!(line_index_at(&lines, Duration::from_millis(1_999)), Some(0));
        assert_eq!(line_index_at(&lines, Duration::from_secs(2)), Some(1));
        assert_eq!(line_index_at(&lines, Duration::from_secs(30)), Some(2));
    }

    fn assert_line(line: &LyricLine, milliseconds: u64, text: &str) {
        assert_eq!(line.time, Duration::from_millis(milliseconds));
        assert_eq!(line.text, text);
        assert_eq!(line.translation, None);
    }
}

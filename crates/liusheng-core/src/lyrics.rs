use std::path::{Path, PathBuf};

use lofty::prelude::TaggedFileExt;
use lofty::tag::{ItemKey, Tag};

use crate::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricLine {
    pub start_ms: Option<u64>,
    pub text: String,
}

/// A display cue keeps simultaneous original/secondary lines in one layout.
/// Timing remains in milliseconds; no word timings or translations are invented.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct LyricCue {
    pub time: Option<u64>,
    pub text: String,
    pub secondary: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lyrics {
    lines: Vec<LyricLine>,
    synchronized: bool,
}

impl Lyrics {
    /// 同名 LRC 优先，音频标签中的 LYRICS 与 UNSYNCEDLYRICS 作为回退。
    pub fn load(audio_path: &Path) -> Result<Option<Self>> {
        if let Some(sidecar) = find_sidecar(audio_path) {
            let text = std::fs::read_to_string(sidecar)?;
            if let Some(lyrics) = parse_text(&text) {
                return Ok(Some(lyrics));
            }
        }

        let tagged = lofty::read_from_path(audio_path)?;
        let primary = tagged.primary_tag().and_then(lyrics_from_tag);
        Ok(primary.or_else(|| tagged.tags().iter().find_map(lyrics_from_tag)))
    }

    pub fn lines(&self) -> &[LyricLine] {
        &self.lines
    }

    pub fn display_cues(&self) -> Vec<LyricCue> {
        let mut cues: Vec<LyricCue> = Vec::new();
        for line in &self.lines {
            if let Some(last) = cues.last_mut()
                && line.start_ms.is_some()
                && last.time == line.start_ms
            {
                // Exact duplicate lines add no information; preserve other simultaneous text.
                if !line.text.is_empty()
                    && line.text != last.text
                    && !last.secondary.lines().any(|text| text == line.text)
                {
                    if last.text.is_empty() {
                        last.text = line.text.clone();
                    } else {
                        if !last.secondary.is_empty() {
                            last.secondary.push('\n');
                        }
                        last.secondary.push_str(&line.text);
                    }
                }
            } else {
                cues.push(LyricCue {
                    time: line.start_ms,
                    text: line.text.clone(),
                    secondary: String::new(),
                });
            }
        }
        cues
    }

    pub fn is_synchronized(&self) -> bool {
        self.synchronized
    }

    pub fn active_index(&self, position_ms: u64) -> Option<usize> {
        if !self.synchronized {
            return None;
        }
        self.lines
            .partition_point(|line| line.start_ms.is_some_and(|start| start <= position_ms))
            .checked_sub(1)
    }
}

fn find_sidecar(audio_path: &Path) -> Option<PathBuf> {
    let direct = audio_path.with_extension("lrc");
    if direct.is_file() {
        return Some(direct);
    }

    let stem = audio_path.file_stem()?;
    std::fs::read_dir(audio_path.parent()?)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && path.file_stem() == Some(stem)
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("lrc"))
        })
}

fn lyrics_from_tag(tag: &Tag) -> Option<Lyrics> {
    tag.get_strings(ItemKey::Lyrics)
        .find_map(parse_text)
        .or_else(|| {
            tag.get_strings(ItemKey::UnsyncLyrics)
                .find_map(parse_plain_text)
        })
}

fn parse_text(text: &str) -> Option<Lyrics> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut timed = Vec::new();
    let mut plain = Vec::new();
    let mut offset_ms = 0_i64;

    for line in text.lines() {
        let (timestamps, content, offset) = parse_lrc_line(line);
        if let Some(offset) = offset {
            offset_ms = offset;
        }
        if timestamps.is_empty() {
            let content = content.trim();
            if !content.is_empty() && !is_metadata_line(content) {
                plain.push(content.to_owned());
            }
            continue;
        }
        for start_ms in timestamps {
            timed.push((start_ms, content.trim().to_owned()));
        }
    }

    if !timed.is_empty() {
        let mut lines = timed
            .into_iter()
            .map(|(start_ms, text)| LyricLine {
                start_ms: Some(start_ms.saturating_add_signed(offset_ms)),
                text,
            })
            .collect::<Vec<_>>();
        lines.sort_by_key(|line| line.start_ms);
        return Some(Lyrics {
            lines,
            synchronized: true,
        });
    }

    lyrics_from_plain_lines(plain)
}

fn parse_plain_text(text: &str) -> Option<Lyrics> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    lyrics_from_plain_lines(
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

fn lyrics_from_plain_lines(lines: Vec<String>) -> Option<Lyrics> {
    if lines.is_empty() {
        return None;
    }
    Some(Lyrics {
        lines: lines
            .into_iter()
            .map(|text| LyricLine {
                start_ms: None,
                text,
            })
            .collect(),
        synchronized: false,
    })
}

fn parse_lrc_line(line: &str) -> (Vec<u64>, &str, Option<i64>) {
    let mut rest = line.trim_start();
    let mut timestamps = Vec::new();
    let mut offset = None;

    while let Some(after_open) = rest.strip_prefix('[') {
        let Some(close) = after_open.find(']') else {
            break;
        };
        let tag = &after_open[..close];
        let mut recognized = false;
        if let Some(timestamp) = parse_timestamp(tag) {
            timestamps.push(timestamp);
            recognized = true;
        } else if let Some((key, value)) = tag.split_once(':') {
            if key.eq_ignore_ascii_case("offset") {
                if let Ok(value) = value.trim().parse::<i64>() {
                    offset = Some(value);
                }
                recognized = true;
            } else if is_metadata_key(key) {
                recognized = true;
            }
        }
        if !recognized {
            break;
        }
        rest = &after_open[close + 1..];
    }

    (timestamps, rest, offset)
}

fn parse_timestamp(tag: &str) -> Option<u64> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes = minutes.parse::<u64>().ok()?;
    let (seconds, fraction) = seconds.split_once(['.', ',']).unwrap_or((seconds, ""));
    let seconds = seconds.parse::<u64>().ok()?;
    if seconds >= 60 {
        return None;
    }
    let fraction = fraction.chars().take(3).collect::<String>();
    if !fraction.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let fraction_ms = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().ok()? * 100,
        2 => fraction.parse::<u64>().ok()? * 10,
        _ => fraction.parse::<u64>().ok()?,
    };
    minutes
        .checked_mul(60_000)?
        .checked_add(seconds * 1_000)?
        .checked_add(fraction_ms)
}

fn is_metadata_line(line: &str) -> bool {
    let Some(tag) = line
        .strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
    else {
        return false;
    };
    tag.split_once(':')
        .is_some_and(|(key, _)| is_metadata_key(key))
}

fn is_metadata_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "al" | "ar" | "au" | "by" | "length" | "offset" | "re" | "ti" | "tool" | "ve"
    )
}

#[cfg(test)]
mod tests {
    use lofty::config::WriteOptions;
    use lofty::prelude::TagExt;
    use lofty::tag::TagType;

    use super::*;

    #[test]
    fn simultaneous_lines_form_one_display_cue_without_losing_raw_timing() {
        let lyrics =
            parse_text("[00:01]Hello\n[00:01]你好\n[00:01]你好\n[00:03]\n[00:05]Next").unwrap();
        let cues = lyrics.display_cues();
        assert_eq!(cues.len(), 3);
        assert_eq!(cues[0].time, Some(1000));
        assert_eq!(cues[0].text, "Hello");
        assert_eq!(cues[0].secondary, "你好");
        assert!(cues[1].text.is_empty()); // Explicit interlude retained.
        assert_eq!(lyrics.lines().len(), 5);
        assert_eq!(lyrics.active_index(1000), Some(2));
    }

    #[test]
    fn untimed_lines_remain_independent_and_markup_is_preserved_as_text() {
        let cues = parse_text("<b>第一行</b>\nsecond line\n第三行")
            .unwrap()
            .display_cues();
        assert_eq!(cues.len(), 3);
        assert!(
            cues.iter()
                .all(|cue| cue.time.is_none() && cue.secondary.is_empty())
        );
        assert_eq!(cues[0].text, "<b>第一行</b>");
    }

    #[test]
    fn cue_grouping_retains_nonempty_line_at_shared_interlude_timestamp() {
        let cues = parse_text("[00:01]\n[00:01]原文\n[00:01]译文\n[00:01]附文")
            .unwrap()
            .display_cues();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "原文");
        assert_eq!(cues[0].secondary, "译文\n附文");
    }

    #[test]
    fn offsets_saturate_without_wrapping_large_unsigned_timestamps() {
        let lyrics = parse_text("[offset:-9223372036854775808]\n[00:01]x").unwrap();
        assert_eq!(lyrics.lines()[0].start_ms, Some(0));
        let lyrics = parse_text("[offset:9223372036854775807]\n[00:01]x").unwrap();
        assert_eq!(lyrics.lines()[0].start_ms, Some(i64::MAX as u64 + 1000));
    }

    #[test]
    fn parses_multiple_timestamps_fraction_precision_and_offset() {
        let lyrics =
            parse_text("[ar:歌手]\n[offset:-250]\n[00:01.2][00:02.345]第一行\n[00:04,05]第二行")
                .unwrap();

        assert!(lyrics.is_synchronized());
        assert_eq!(
            lyrics.lines(),
            [
                LyricLine {
                    start_ms: Some(950),
                    text: "第一行".to_owned(),
                },
                LyricLine {
                    start_ms: Some(2_095),
                    text: "第一行".to_owned(),
                },
                LyricLine {
                    start_ms: Some(3_800),
                    text: "第二行".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn parses_bom_crlf_plain_text() {
        let lyrics = parse_text("\u{feff}[Verse]\r\n第一行\r\n\r\n第二行\r\n").unwrap();

        assert!(!lyrics.is_synchronized());
        assert_eq!(lyrics.lines()[0].text, "[Verse]");
        assert_eq!(lyrics.lines()[1].text, "第一行");
        assert_eq!(lyrics.lines()[2].text, "第二行");
    }

    #[test]
    fn active_index_tracks_timestamp_boundaries() {
        let lyrics = parse_text("[00:01.00]一\n[00:03.00]二").unwrap();

        assert_eq!(lyrics.active_index(999), None);
        assert_eq!(lyrics.active_index(1_000), Some(0));
        assert_eq!(lyrics.active_index(2_999), Some(0));
        assert_eq!(lyrics.active_index(3_000), Some(1));
    }

    #[test]
    fn sidecar_is_loaded_before_the_audio_file() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("song.flac");
        std::fs::write(dir.path().join("song.LRC"), "[00:00.50]侧载歌词").unwrap();

        let lyrics = Lyrics::load(&audio).unwrap().unwrap();

        assert_eq!(lyrics.lines()[0].text, "侧载歌词");
        assert_eq!(lyrics.lines()[0].start_ms, Some(500));
    }

    #[test]
    fn embedded_lyrics_prefer_synchronized_text() {
        let mut tag = Tag::new(TagType::VorbisComments);
        assert!(tag.insert_text(ItemKey::UnsyncLyrics, "纯文本".to_owned()));
        assert!(tag.insert_text(ItemKey::Lyrics, "[00:02]同步文本".to_owned()));

        let lyrics = lyrics_from_tag(&tag).unwrap();

        assert!(lyrics.is_synchronized());
        assert_eq!(lyrics.lines()[0].text, "同步文本");
    }

    #[test]
    fn loads_unsynchronized_lyrics_from_an_audio_tag() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("tagged.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&audio, spec).unwrap();
        writer.write_sample(0_i16).unwrap();
        writer.write_sample(0_i16).unwrap();
        writer.finalize().unwrap();

        let mut tag = Tag::new(TagType::Id3v2);
        assert!(tag.insert_text(ItemKey::UnsyncLyrics, "内嵌第一行\n内嵌第二行".to_owned()));
        tag.save_to_path(&audio, WriteOptions::default()).unwrap();

        let lyrics = Lyrics::load(&audio).unwrap().unwrap();

        assert!(!lyrics.is_synchronized());
        assert_eq!(lyrics.lines()[0].text, "内嵌第一行");
        assert_eq!(lyrics.lines()[1].text, "内嵌第二行");
    }
}

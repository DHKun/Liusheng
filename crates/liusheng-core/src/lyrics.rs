use std::io::Read;
use std::path::{Path, PathBuf};

use lofty::prelude::TaggedFileExt;
use lofty::tag::{ItemKey, Tag};

use crate::{Error, Result};

mod timing;

// Limits cover sidecars and embedded lyrics before allocation in the QML/JS heap.
const MAX_LYRIC_BYTES: usize = 1024 * 1024;
const MAX_LYRIC_LINES: usize = 10_000;
const MAX_LINE_BYTES: usize = 8192;
const MAX_EXPANDED_BYTES: usize = 2 * 1024 * 1024;

/// Source word/syllable timing. Offsets use UTF-16 code units for Qt/QML.
/// An absent end records a source that supplies only the word's onset.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct LyricWord {
    pub offset: u32,
    pub length: u32,
    pub start: u64,
    pub end: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LyricLine {
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub text: String,
    pub secondary: String,
    pub words: Vec<LyricWord>,
}

/// A display cue keeps simultaneous original/secondary lines in one layout.
/// Timing remains source-derived. Estimated highlighting belongs to the display layer.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct LyricCue {
    pub time: Option<u64>,
    pub end: Option<u64>,
    pub text: String,
    pub secondary: String,
    pub words: Vec<LyricWord>,
    pub timing: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lyrics {
    lines: Vec<LyricLine>,
    synchronized: bool,
}

impl Lyrics {
    /// Shared bounded parser for local and downloaded LRC or plain text.
    pub fn from_text(text: &str) -> Result<Self> {
        parse_text(text).ok_or_else(|| Error::Other("歌词为空、格式错误或超出文本限制".into()))
    }

    /// 同名逐字文件、LRC 按优先级读取，音频标签中的歌词作为回退。
    pub fn load(audio_path: &Path) -> Result<Option<Self>> {
        let mut sidecar_error = None;
        for sidecar in find_sidecars(audio_path) {
            match Self::read_file(&sidecar) {
                Ok((lyrics, _)) => return Ok(Some(lyrics)),
                Err(error) => sidecar_error = Some(format!("外部歌词读取失败：{error}")),
            }
        }
        let embedded = lofty::read_from_path(audio_path).map(|tagged| {
            tagged
                .primary_tag()
                .and_then(lyrics_from_tag)
                .or_else(|| tagged.tags().iter().find_map(lyrics_from_tag))
        });
        match (embedded, sidecar_error) {
            (Ok(Some(lyrics)), warning) => {
                if let Some(warning) = warning {
                    eprintln!("[lyrics] {warning}；已使用内嵌歌词");
                }
                Ok(Some(lyrics))
            }
            (Ok(None), Some(error)) => Err(Error::Other(error)),
            (Err(error), Some(sidecar)) => Err(Error::Other(format!(
                "{sidecar}；内嵌歌词读取失败：{error}"
            ))),
            (result, None) => result.map_err(Error::from),
        }
    }

    pub fn lines(&self) -> &[LyricLine] {
        &self.lines
    }

    /// Validate a local lyric file for the asynchronous import service.
    pub fn read_file(path: &Path) -> Result<(Self, String)> {
        let text = read_sidecar(path)?;
        let lyrics = Self::from_text(&text)?;
        let rich = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                ["ttml", "yrc", "qrc", "alrc"]
                    .iter()
                    .any(|kind| ext.eq_ignore_ascii_case(kind))
            });
        if rich && !lyrics.is_synchronized() {
            return Err(Error::Other(
                "请选择含时间信息的明文歌词；加密 QRC 请先导出为 LRC 或 TTML".into(),
            ));
        }
        Ok((lyrics, text))
    }

    pub fn has_word_timing(&self) -> bool {
        self.lines.iter().any(|line| !line.words.is_empty())
    }

    pub fn display_cues(&self) -> Vec<LyricCue> {
        let mut cues: Vec<LyricCue> = Vec::new();
        for line in &self.lines {
            if let Some(last) = cues.last_mut()
                && line.start_ms.is_some()
                && last.time == line.start_ms
            {
                if last.text.is_empty()
                    || (last.text == line.text && last.words.is_empty() && !line.words.is_empty())
                {
                    last.text = line.text.clone();
                    last.words = line.words.clone();
                    last.end = line.end_ms;
                } else if !line.text.is_empty()
                    && line.text != last.text
                    && !last.secondary.lines().any(|s| s == line.text)
                {
                    append_secondary(&mut last.secondary, &line.text);
                }
                append_secondary(&mut last.secondary, &line.secondary);
                last.timing = timing_kind(last.time, &last.words);
            } else {
                cues.push(LyricCue {
                    time: line.start_ms,
                    end: line.end_ms,
                    text: line.text.clone(),
                    secondary: line.secondary.clone(),
                    words: line.words.clone(),
                    timing: timing_kind(line.start_ms, &line.words),
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

fn valid_text_size(text: &str) -> bool {
    text.len() <= MAX_LYRIC_BYTES
        && text.lines().take(MAX_LYRIC_LINES + 1).count() <= MAX_LYRIC_LINES
        && text.lines().all(|line| line.len() <= MAX_LINE_BYTES)
}

fn read_sidecar(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((MAX_LYRIC_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_LYRIC_BYTES {
        return Err(Error::Other("歌词文件超过 1 MiB".into()));
    }
    let text = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if bytes.len() % 2 != 0 {
            return Err(Error::Other("UTF-16 歌词长度无效".into()));
        }
        let little = bytes[0] == 0xff;
        let units = bytes[2..].as_chunks::<2>().0.iter().map(|b| {
            if little {
                u16::from_le_bytes([b[0], b[1]])
            } else {
                u16::from_be_bytes([b[0], b[1]])
            }
        });
        char::decode_utf16(units)
            .collect::<std::result::Result<String, _>>()
            .map_err(|e| Error::Other(format!("UTF-16 歌词编码错误：{e}")))?
    } else {
        String::from_utf8(bytes).map_err(|e| Error::Other(format!("UTF-8 歌词编码错误：{e}")))?
    };
    if !valid_input_size(&text) {
        return Err(Error::Other("歌词超过文本、行数或单行长度限制".into()));
    }
    Ok(text)
}

fn find_sidecars(audio_path: &Path) -> Vec<PathBuf> {
    const EXTENSIONS: &[&str] = &["ttml", "yrc", "qrc", "alrc", "lrc"];
    let mut found = Vec::new();
    for ext in EXTENSIONS {
        let direct = audio_path.with_extension(ext);
        if direct.is_file() {
            found.push(direct);
        }
    }
    let Some(stem) = audio_path.file_stem() else {
        return found;
    };
    if let Some(parent) = audio_path.parent()
        && let Ok(entries) = std::fs::read_dir(parent)
    {
        let mut matches = entries
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| p.file_stem() == Some(stem) && p.is_file())
            .filter_map(|p| {
                let index = EXTENSIONS.iter().position(|ext| {
                    p.extension()
                        .and_then(|x| x.to_str())
                        .is_some_and(|x| x.eq_ignore_ascii_case(ext))
                })?;
                Some((index, p))
            })
            .collect::<Vec<_>>();
        matches.sort();
        for (_, path) in matches {
            if !found.contains(&path) {
                found.push(path);
            }
        }
    }
    found.sort_by_key(|p| {
        EXTENSIONS.iter().position(|ext| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case(ext))
        })
    });
    found
}

fn timing_kind(time: Option<u64>, words: &[LyricWord]) -> &'static str {
    if time.is_none() {
        "plain"
    } else if words.is_empty() {
        "line"
    } else if words.iter().all(|word| word.end.is_some()) {
        "word"
    } else {
        "word-start"
    }
}

fn append_secondary(target: &mut String, text: &str) {
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if target.lines().any(|existing| existing == line) {
            continue;
        }
        if !target.is_empty() {
            target.push('\n');
        }
        target.push_str(line);
    }
}

fn lyrics_from_tag(tag: &Tag) -> Option<Lyrics> {
    tag.get_strings(ItemKey::Lyrics)
        .find_map(parse_text)
        .or_else(|| {
            tag.get_strings(ItemKey::UnsyncLyrics)
                .find_map(parse_plain_text)
        })
}

fn valid_input_size(text: &str) -> bool {
    text.len() <= MAX_LYRIC_BYTES
        && !text.contains('\0')
        && (timing::is_xml(text) || valid_text_size(text))
}

fn parse_text(text: &str) -> Option<Lyrics> {
    if !valid_input_size(text) {
        return None;
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if timing::is_xml(text) {
        return timing::parse_xml(text);
    }
    let mut lines = Vec::new();
    let mut plain = Vec::new();
    let mut offset_ms = 0_i64;
    let mut expanded_bytes = 0usize;
    let mut word_count = 0usize;
    for raw in text.lines() {
        if timing::has_duration_header(raw) {
            let line = timing::duration_line(raw)?;
            expanded_bytes += line.text.len();
            word_count += line.words.len();
            lines.push(line);
        } else {
            let (timestamps, content, offset) = parse_lrc_line(raw);
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
            let original_start = timestamps[0];
            let prototype = timing::enhanced_line(original_start, content);
            for start in timestamps {
                let mut line = prototype.clone();
                timing::shift_line(&mut line, i128::from(start) - i128::from(original_start));
                expanded_bytes = expanded_bytes.saturating_add(line.text.len());
                word_count += line.words.len();
                lines.push(line);
            }
        }
        if lines.len() > MAX_LYRIC_LINES
            || word_count > 50000
            || expanded_bytes > MAX_EXPANDED_BYTES
        {
            return None;
        }
    }
    if lines.is_empty() {
        return lyrics_from_plain_lines(plain);
    }
    for line in &mut lines {
        timing::shift_line(line, i128::from(offset_ms));
    }
    lines.sort_by_key(|line| line.start_ms);
    Some(Lyrics {
        lines,
        synchronized: true,
    })
}

fn parse_plain_text(text: &str) -> Option<Lyrics> {
    if !valid_text_size(text) {
        return None;
    }
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
                ..Default::default()
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
                    ..Default::default()
                },
                LyricLine {
                    start_ms: Some(2_095),
                    text: "第一行".to_owned(),
                    ..Default::default()
                },
                LyricLine {
                    start_ms: Some(3_800),
                    text: "第二行".to_owned(),
                    ..Default::default()
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

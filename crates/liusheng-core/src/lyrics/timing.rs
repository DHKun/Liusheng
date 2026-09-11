//! Bounded, source-derived word timing for Enhanced LRC, plaintext YRC/QRC and lyric TTML.
use super::{LyricLine, LyricWord, Lyrics};
mod xml;
pub(super) use xml::{is_xml, parse_xml};
const MAX_TIME: u64 = 7 * 24 * 60 * 60 * 1000;
const MAX_WORDS_PER_LINE: usize = 2048;

fn stamp(text: &str) -> Option<u64> {
    if text.len() > 24
        || !text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ':' | '.' | ','))
    {
        return None;
    }
    super::parse_timestamp(text).filter(|t| *t <= MAX_TIME)
}

pub(super) fn shift_line(line: &mut LyricLine, shift: i128) {
    let shift_time = |time: u64| (i128::from(time) + shift).clamp(0, i128::from(u64::MAX)) as u64;
    line.start_ms = line.start_ms.map(shift_time);
    line.end_ms = line.end_ms.map(shift_time);
    for word in &mut line.words {
        word.start = shift_time(word.start);
        word.end = word.end.map(shift_time);
    }
}

fn append_word(line: &mut LyricLine, text: &str, start: u64, end: Option<u64>) {
    let offset = line.text.encode_utf16().count() as u32;
    line.text.push_str(text);
    let length = text.encode_utf16().count() as u32;
    if length > 0 {
        line.words.push(LyricWord {
            offset,
            length,
            start,
            end,
        });
    }
}

fn trim_line(line: &mut LyricLine) {
    let leading = line.text[..line.text.len() - line.text.trim_start().len()]
        .encode_utf16()
        .count() as u32;
    let trimmed = line.text.trim().to_owned();
    let end = leading + trimmed.encode_utf16().count() as u32;
    line.words.retain_mut(|word| {
        let from = word.offset.max(leading);
        let to = word.offset.saturating_add(word.length).min(end);
        if to <= from {
            return false;
        }
        word.offset = from - leading;
        word.length = to - from;
        true
    });
    line.text = trimmed;
}

fn valid_words(line: &LyricLine) -> bool {
    let begin = line.start_ms.unwrap_or(0);
    let mut previous = begin;
    let length = line.text.encode_utf16().count() as u32;
    line.words.len() <= MAX_WORDS_PER_LINE
        && line.words.iter().all(|word| {
            let valid = word.start >= previous
                && word.start <= MAX_TIME
                && word
                    .end
                    .is_none_or(|end| end >= word.start && end <= MAX_TIME)
                && line
                    .end_ms
                    .is_none_or(|end| word.start <= end && word.end.is_none_or(|stop| stop <= end))
                && word
                    .offset
                    .checked_add(word.length)
                    .is_some_and(|to| to <= length);
            previous = word.start;
            valid
        })
}

/// A2 timestamps are absolute onsets. Consecutive markers preserve an explicit pause.
/// A final marker without text records the previous word's end; otherwise it stays unknown.
pub(super) fn enhanced_line(start: u64, content: &str) -> LyricLine {
    let mut markers = Vec::new();
    let mut scan = 0;
    while let Some(index) = content[scan..].find('<') {
        let begin = scan + index;
        let Some(close) = content[begin + 1..].find('>') else {
            break;
        };
        let end = begin + close + 2;
        if let Some(time) = stamp(&content[begin + 1..end - 1]) {
            markers.push((begin, end, time));
        }
        scan = end;
    }
    let mut line = LyricLine {
        start_ms: Some(start),
        ..Default::default()
    };
    if markers.is_empty() {
        line.text = content.trim().into();
        return line;
    }
    let prefix = &content[..markers[0].0];
    append_word(&mut line, prefix, start, Some(markers[0].2));
    for (index, &(_, text_start, time)) in markers.iter().enumerate() {
        let next = markers.get(index + 1);
        let segment = &content[text_start..next.map_or(content.len(), |m| m.0)];
        append_word(&mut line, segment, time, next.map(|m| m.2));
    }
    if content[markers.last().unwrap().1..].trim().is_empty() {
        line.end_ms = Some(markers.last().unwrap().2);
    }
    trim_line(&mut line);
    let monotonic = markers.windows(2).all(|pair| pair[0].2 <= pair[1].2);
    if !monotonic || !valid_words(&line) {
        // Malformed word data falls back to readable, cleaned line text.
        line.words.clear();
        line.end_ms = None;
    }
    line
}

fn tuple(text: &str, fields: usize) -> Option<Vec<u64>> {
    let values = text
        .split(',')
        .map(|part| {
            let part = part.trim();
            if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            part.parse::<u64>().ok().filter(|n| *n <= MAX_TIME)
        })
        .collect::<Option<Vec<_>>>()?;
    (values.len() == fields).then_some(values)
}

pub(super) fn has_duration_header(text: &str) -> bool {
    text.trim_start()
        .strip_prefix('[')
        .and_then(|text| text.split_once(']'))
        .is_some_and(|(header, _)| header.contains(',') && !header.contains(':'))
}

pub(super) fn duration_line(raw: &str) -> Option<LyricLine> {
    let (header, body) = raw.trim().strip_prefix('[')?.split_once(']')?;
    let header = tuple(header, 2)?;
    let start = header[0];
    let end = start.checked_add(header[1]).filter(|t| *t <= MAX_TIME)?;
    let yrc = body
        .strip_prefix('(')
        .and_then(|x| x.split_once(')'))
        .is_some_and(|(x, _)| tuple(x, 3).is_some());
    let fields = if yrc { 3 } else { 2 };
    let mut markers = Vec::new();
    let mut scan = 0;
    while let Some(index) = body[scan..].find('(') {
        let begin = scan + index;
        let Some(close) = body[begin + 1..].find(')') else {
            break;
        };
        let text_start = begin + close + 2;
        if let Some(values) = tuple(&body[begin + 1..text_start - 1], fields) {
            markers.push((
                begin,
                text_start,
                values[0],
                values[0].checked_add(values[1])?,
            ));
        }
        scan = text_start;
    }
    let mut line = LyricLine {
        start_ms: Some(start),
        end_ms: Some(end),
        ..Default::default()
    };
    if markers.is_empty() {
        line.text = body.trim().into();
        return Some(line);
    }
    if yrc {
        line.text.push_str(&body[..markers[0].0]);
        for (index, &(_, offset, begin, stop)) in markers.iter().enumerate() {
            let text_end = markers.get(index + 1).map_or(body.len(), |x| x.0);
            append_word(&mut line, &body[offset..text_end], begin, Some(stop));
        }
    } else {
        let mut offset = 0;
        for &(text_end, next, begin, stop) in &markers {
            append_word(&mut line, &body[offset..text_end], begin, Some(stop));
            offset = next;
        }
        line.text.push_str(&body[offset..]);
    }
    trim_line(&mut line);
    if !valid_words(&line) {
        line.words.clear();
    }
    Some(line)
}

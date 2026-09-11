//! Bounded lyric TTML import with explicit media-clock and parallel offset timing.
use super::{LyricLine, Lyrics, MAX_TIME, append_word, trim_line};
use roxmltree::{Document, Node, ParsingOptions};
const ITUNES: &str = "http://music.apple.com/lyric-ttml-internal";
const METADATA: &str = "http://www.w3.org/ns/ttml#metadata";
const XML: &str = "http://www.w3.org/XML/1998/namespace";

pub(crate) fn is_xml(text: &str) -> bool {
    let text = text.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
    text.starts_with("<?xml")
        || text.starts_with("<tt")
        || text.starts_with("<QrcInfos")
        || text.starts_with("<!DOCTYPE")
}

fn scalar(text: &str, scale: u64) -> Option<u64> {
    if !text.bytes().any(|c| c.is_ascii_digit())
        || !text.bytes().all(|c| c.is_ascii_digit() || c == b'.')
    {
        return None;
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if fraction.len() > 9 || fraction.contains('.') {
        return None;
    }
    let whole = if whole.is_empty() {
        0
    } else {
        whole.parse::<u64>().ok()?
    };
    let fraction_value = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u64>().ok()?
    };
    whole
        .checked_mul(scale)?
        .checked_add(fraction_value.checked_mul(scale)? / 10u64.pow(fraction.len() as u32))
        .filter(|time| *time <= MAX_TIME)
}

fn time(text: &str) -> Option<u64> {
    let text = text.trim();
    if let Some(value) = text.strip_suffix("ms") {
        return scalar(value, 1);
    }
    for (suffix, scale) in [('s', 1000), ('m', 60000), ('h', 3600000)] {
        if let Some(value) = text.strip_suffix(suffix) {
            return scalar(value, scale);
        }
    }
    let parts = text.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [seconds] => scalar(seconds, 1000),
        [minutes, seconds] => {
            let seconds = scalar(seconds, 1000).filter(|n| *n < 60000)?;
            scalar(minutes, 60000)?
                .checked_add(seconds)
                .filter(|n| *n <= MAX_TIME)
        }
        [hours, minutes, seconds] => {
            let seconds = scalar(seconds, 1000).filter(|n| *n < 60000)?;
            let minutes = scalar(minutes, 60000).filter(|n| *n < 3600000)?;
            scalar(hours, 3600000)?
                .checked_add(minutes)?
                .checked_add(seconds)
                .filter(|n| *n <= MAX_TIME)
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct Interval {
    start: u64,
    end: Option<u64>,
}
fn interval(node: Node<'_, '_>, parent: Interval, absolute: bool) -> Option<Interval> {
    let origin = if absolute { 0 } else { parent.start };
    let start = match node.attribute("begin") {
        Some(begin) => origin.checked_add(time(begin)?)?,
        None => parent.start,
    };
    let mut end = match node.attribute("end") {
        Some(end) => Some(origin.checked_add(time(end)?)?),
        None => parent.end,
    };
    if let Some(dur) = node.attribute("dur") {
        let stop = start.checked_add(time(dur)?)?;
        end = Some(end.map_or(stop, |end| end.min(stop)));
    }
    if let Some(limit) = parent.end {
        end = Some(end.map_or(limit, |end| end.min(limit)));
    }
    if start > MAX_TIME || end.is_some_and(|end| end < start || end > MAX_TIME) {
        return None;
    }
    Some(Interval { start, end })
}

fn preserved(node: Node<'_, '_>) -> bool {
    node.ancestors()
        .find_map(|n| n.attribute((XML, "space")))
        .is_some_and(|s| s == "preserve")
}
fn text_piece(text: &str, preserve: bool) -> String {
    if preserve {
        return text.to_owned();
    }
    if text.trim().is_empty() && text.contains(['\n', '\r']) {
        return String::new();
    }
    let mut result = String::new();
    for c in text.chars() {
        if c.is_whitespace() {
            if !result.ends_with(' ') {
                result.push(' ');
            }
        } else {
            result.push(c);
        }
    }
    result
}
fn auxiliary(node: Node<'_, '_>) -> String {
    let mut result = String::new();
    for child in node.descendants() {
        if child.is_text() {
            result.push_str(&text_piece(child.text().unwrap_or(""), preserved(child)));
        } else if child.has_tag_name("br") {
            result.push('\n');
        }
    }
    result.trim().to_owned()
}
fn content(
    node: Node<'_, '_>,
    timing: Interval,
    absolute: bool,
    timed: bool,
    line: &mut LyricLine,
    depth: usize,
) -> Option<()> {
    if depth > 32 {
        return None;
    }
    for child in node.children() {
        if child.is_text() {
            let text = text_piece(child.text().unwrap_or(""), preserved(child));
            if timed {
                append_word(line, &text, timing.start, timing.end);
            } else {
                line.text.push_str(&text);
            }
        } else if child.is_element() {
            let role = child
                .attribute((METADATA, "role"))
                .or_else(|| child.attribute("role"))
                .unwrap_or("");
            if role
                .split_whitespace()
                .any(|r| matches!(r, "x-translation" | "x-roman" | "x-bg"))
            {
                super::super::append_secondary(&mut line.secondary, &auxiliary(child));
                continue;
            }
            if child.has_tag_name("br") {
                line.text.push('\n');
                continue;
            }
            if child.has_tag_name("metadata") {
                continue;
            }
            let next = interval(child, timing, absolute)?;
            let word = timed || (child.has_tag_name("span") && child.attribute("begin").is_some());
            content(child, next, absolute, word, line, depth + 1)?;
        }
        if line.text.len() > super::super::MAX_LINE_BYTES
            || line.secondary.len() > super::super::MAX_LINE_BYTES
            || line.words.len() > super::MAX_WORDS_PER_LINE
        {
            return None;
        }
    }
    Some(())
}

pub(crate) fn parse_xml(text: &str) -> Option<Lyrics> {
    if text.contains("<!DOCTYPE") {
        return None;
    }
    let doc = Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: 60000,
            ..Default::default()
        },
    )
    .ok()?;
    if doc.descendants().any(|node| {
        node.ancestors().take(34).count() >= 34
            || node.attribute("timeContainer").is_some_and(|s| s != "par")
    }) {
        return None;
    }
    if doc.root_element().has_tag_name("QrcInfos") {
        let raw = doc
            .descendants()
            .find_map(|node| node.attribute("LyricContent"))?;
        if is_xml(raw) {
            return None;
        }
        return super::super::parse_text(raw);
    }
    let root = doc.root_element();
    if root.tag_name().name() != "tt"
        || !doc.descendants().any(|node| {
            node.is_element()
                && ["begin", "end", "dur"]
                    .iter()
                    .any(|a| node.attribute(*a).is_some())
        })
    {
        return None;
    }
    // Frame/tick and wall-clock profiles require their own timing conversion.
    if root
        .attribute(("http://www.w3.org/ns/ttml#parameter", "timeBase"))
        .is_some_and(|base| base != "media")
    {
        return None;
    }
    let absolute = root.lookup_prefix(ITUNES).is_some()
        || root.attribute((ITUNES, "timing")).is_some()
        || root.namespaces().any(|ns| ns.name() == Some("amll"));
    let mut lines = Vec::new();
    let mut bytes = 0;
    let mut count = 0;
    for node in doc.descendants().filter(|node| node.has_tag_name("p")) {
        let mut parents = node
            .ancestors()
            .filter(|n| n.is_element())
            .collect::<Vec<_>>();
        parents.reverse();
        let mut bounds = Interval {
            start: 0,
            end: None,
        };
        for ancestor in parents {
            bounds = interval(ancestor, bounds, absolute)?;
        }
        let mut line = LyricLine {
            start_ms: Some(bounds.start),
            end_ms: bounds.end,
            ..Default::default()
        };
        content(node, bounds, absolute, false, &mut line, 0)?;
        trim_line(&mut line);
        // Styling may split one timed span into several text nodes. Preserve a
        // single reveal interval across these adjacent pieces of the same word.
        let mut words: Vec<super::LyricWord> = Vec::with_capacity(line.words.len());
        for word in line.words.drain(..) {
            if let Some(last) = words.last_mut()
                && last.start == word.start
                && last.end == word.end
                && last.offset + last.length == word.offset
            {
                last.length += word.length;
            } else {
                words.push(word);
            }
        }
        line.words = words;
        if line
            .words
            .iter()
            .any(|w| w.start < bounds.start || w.end.is_some_and(|e| e < w.start))
        {
            return None;
        }
        if node.attribute("begin").is_none()
            && let Some(first) = line.words.iter().map(|w| w.start).min()
        {
            line.start_ms = Some(first);
        }
        if line.end_ms.is_none() {
            line.end_ms = line.words.iter().filter_map(|w| w.end).max();
        }
        bytes += line.text.len() + line.secondary.len();
        count += line.words.len();
        if lines.len() >= super::super::MAX_LYRIC_LINES
            || count > 50000
            || bytes > super::super::MAX_EXPANDED_BYTES
        {
            return None;
        }
        lines.push(line);
    }
    if lines.is_empty() {
        return None;
    }
    lines.sort_by_key(|line| line.start_ms);
    Some(Lyrics {
        lines,
        synchronized: true,
    })
}

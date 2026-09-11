use liusheng_core::lyrics::Lyrics;

#[test]
fn enhanced_lrc_preserves_exact_onsets_ends_pauses_and_utf16_offsets() {
    let lyrics =
        Lyrics::from_text("[00:01]<00:01>你<00:01.400><00:02>好😀<00:03>\n[00:04]next").unwrap();
    let cues = lyrics.display_cues();
    assert_eq!(cues[0].text, "你好😀");
    assert_eq!(cues[0].timing, "word");
    assert_eq!(cues[0].words.len(), 2);
    assert_eq!(cues[0].words[0].end, Some(1400));
    assert_eq!(cues[0].words[1].start, 2000);
    assert_eq!(cues[0].words[1].offset, 1);
    assert_eq!(cues[0].words[1].length, 3);
    assert_eq!(cues[0].end, Some(3000));
    assert_eq!(cues[1].timing, "line");
}
#[test]
fn unknown_final_word_end_stays_unknown() {
    let cue = Lyrics::from_text("[00:01]<00:01>long <00:02>note")
        .unwrap()
        .display_cues()
        .remove(0);
    assert_eq!(cue.words[1].end, None);
    assert_eq!(cue.end, None);
    assert_eq!(cue.timing, "word-start");
}
#[test]
fn repeated_lines_and_file_offset_apply_to_every_word_once() {
    let cues = Lyrics::from_text("[offset:150]\n[00:01][00:05]<00:01>one<00:02>two<00:03>")
        .unwrap()
        .display_cues();
    assert_eq!(cues[1].time, Some(5150));
    assert_eq!(cues[1].words[0].start, 5150);
    assert_eq!(cues[1].words[1].end, Some(7150));
}
#[test]
fn duration_formats_preserve_absolute_times_and_literal_parentheses() {
    let yrc = Lyrics::from_text("[1000,4000](1000,500,0)Hello (2000,2000,0)世界").unwrap();
    let qrc = Lyrics::from_text("[1000,4000]Hello (1000,500)世界(2000,2000)").unwrap();
    assert_eq!(yrc, qrc);
    let cue = qrc.display_cues().remove(0);
    assert_eq!(cue.text, "Hello 世界");
    assert_eq!(cue.words[0].start, 1000);
    assert_eq!(cue.words[1].end, Some(4000));
    assert_eq!(cue.end, Some(5000));
    assert_eq!(
        Lyrics::from_text("[1000,1000](hello)(1000,1000)")
            .unwrap()
            .lines()[0]
            .text,
        "(hello)"
    );
}
#[test]
fn malformed_word_times_fall_back_to_clean_readable_line() {
    for raw in [
        "[00:01]<00:02>a<00:01>b",
        "[1000,1000](900,500,0)early(2000,500,0)late",
    ] {
        let cue = Lyrics::from_text(raw).unwrap().display_cues().remove(0);
        assert!(cue.words.is_empty());
        assert!(!cue.text.contains("00:"));
        assert_eq!(cue.timing, "line");
    }
}
#[test]
fn same_time_translation_does_not_steal_original_word_ranges() {
    let cues = Lyrics::from_text("[00:01]<00:01>Hello<00:02>\n[00:01]你好\n[00:03]")
        .unwrap()
        .display_cues();
    assert_eq!(cues[0].text, "Hello");
    assert_eq!(cues[0].secondary, "你好");
    assert_eq!(cues[0].words.len(), 1);
    assert!(cues[1].text.is_empty());
}
#[test]
fn lyric_ttml_uses_media_clock_and_explicit_auxiliary_roles() {
    let text = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata" xmlns:itunes="http://music.apple.com/lyric-ttml-internal" itunes:timing="Word"><body><div><p begin="00:10.000" end="00:14.000"><span begin="00:10.000" end="00:11.000">Hello </span><span begin="00:12.000" end="00:14.000">世界</span><span ttm:role="x-translation">你好世界</span><span ttm:role="x-roman">hello sekai</span></p></div></body></tt>"#;
    let cues = Lyrics::from_text(text).unwrap().display_cues();
    assert_eq!(cues[0].time, Some(10000));
    assert_eq!(cues[0].words[1].start, 12000);
    assert_eq!(cues[0].words[1].end, Some(14000));
    assert_eq!(cues[0].text, "Hello 世界");
    assert_eq!(cues[0].secondary, "你好世界\nhello sekai");
}
#[test]
fn parallel_ttml_resolves_parent_relative_offsets() {
    let text = r#"<tt><body begin="1s"><div begin="2s"><p begin="3s" dur="4s"><span begin="0.5s" dur="1s">one </span><span begin="2s" end="4s">two</span></p></div></body></tt>"#;
    let cue = Lyrics::from_text(text).unwrap().display_cues().remove(0);
    assert_eq!(cue.time, Some(6000));
    assert_eq!(cue.end, Some(10000));
    assert_eq!(cue.words[0].start, 6500);
    assert_eq!(cue.words[0].end, Some(7500));
    assert_eq!(cue.words[1].start, 8000);
}
#[test]
fn xml_text_is_decoded_but_never_executed_as_markup() {
    let text = r#"<tt><body><p begin="1s" end="4s"><span begin="0s" dur="2s">&lt;b&gt;A &amp; B&lt;/b&gt;</span></p></body></tt>"#;
    let cue = Lyrics::from_text(text).unwrap().display_cues().remove(0);
    assert_eq!(cue.text, "<b>A & B</b>");
    assert!(
        Lyrics::from_text("<b>literal</b>").unwrap().display_cues()[0]
            .words
            .is_empty()
    );
}
#[test]
fn xml_limits_and_unsupported_time_containers_are_explicit() {
    assert!(Lyrics::from_text("<!DOCTYPE tt><tt/>").is_err());
    assert!(Lyrics::from_text("<tt><body timeContainer=\"seq\"><p>word</p></body></tt>").is_err());
    assert!(Lyrics::from_text("<tt><body><p begin=\"NaNs\">word</p></body></tt>").is_err());
    assert!(
        Lyrics::from_text(&format!(
            "<tt>{}text{}</tt>",
            "<span>".repeat(35),
            "</span>".repeat(35)
        ))
        .is_err()
    );
    assert!(Lyrics::from_text(&"a".repeat(1024 * 1024 + 1)).is_err());
}
#[test]
fn plaintext_qrc_xml_wrapper_uses_same_timing_parser() {
    let lyrics = Lyrics::from_text(
        r#"<QrcInfos><Lyric_1 LyricContent="[1000,1000]hello(1000,1000)"/></QrcInfos>"#,
    )
    .unwrap();
    assert_eq!(lyrics.display_cues()[0].words[0].end, Some(2000));
}
#[test]
fn sidecar_priority_and_invalid_rich_sidecar_fallback() {
    let root = tempfile::tempdir().unwrap();
    let audio = root.path().join("fixture.wav");
    std::fs::write(audio.with_extension("lrc"), "[00:01]ordinary").unwrap();
    std::fs::write(audio.with_extension("yrc"), "[1000,1000](1000,1000,0)timed").unwrap();
    assert!(Lyrics::load(&audio).unwrap().unwrap().has_word_timing());
    std::fs::write(audio.with_extension("ttml"), "<tt>broken").unwrap();
    assert!(Lyrics::load(&audio).unwrap().unwrap().has_word_timing());
    std::fs::remove_file(audio.with_extension("yrc")).unwrap();
    assert_eq!(
        Lyrics::load(&audio).unwrap().unwrap().lines()[0].text,
        "ordinary"
    );
}

#[test]
fn rich_file_import_requires_timing_and_keeps_ordinary_plain_text_supported() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("words.qrc");
    std::fs::write(&path, "opaque payload without timings").unwrap();
    assert!(Lyrics::read_file(&path).is_err());
    let path = root.path().join("words.txt");
    std::fs::write(&path, "plain lyrics for reading").unwrap();
    assert!(!Lyrics::read_file(&path).unwrap().0.is_synchronized());
}
#[test]
fn retired_highlight_preference_is_ignored_without_losing_user_settings() {
    use liusheng_core::settings::AppSettings;
    for mode in ["progressive", "accurate", "line"] {
        let input = serde_json::json!({"version":1,"lyric_highlight":mode,"online_lyrics":true,"lyric_offsets":{"song":120}});
        let settings: AppSettings = serde_json::from_value(input).unwrap();
        settings.validate().unwrap();
        assert!(settings.online_lyrics);
        assert_eq!(settings.lyric_offsets["song"], 120);
        assert!(
            serde_json::to_value(&settings)
                .unwrap()
                .get("lyric_highlight")
                .is_none()
        );
    }
}

#[test]
fn ttml_styled_fragments_share_one_word_and_reject_untimed_documents() {
    let text = r#"<tt><body><p begin="1s" end="5s"><span begin="0s" dur="2s">he<span>ll</span>o</span><span begin="3s" dur="1s"> world</span></p></body></tt>"#;
    let cue = Lyrics::from_text(text).unwrap().display_cues().remove(0);
    assert_eq!(cue.text, "hello world");
    assert_eq!(cue.words.len(), 2);
    assert_eq!(cue.words[0].length, 5);
    assert_eq!(cue.words[0].end, Some(3000));
    assert!(Lyrics::from_text("<tt><body><p>untimed document</p></body></tt>").is_err());
    assert!(Lyrics::from_text("<tt><body><p begin=\".s\">invalid time</p></body></tt>").is_err());
}

#[test]
fn structured_lyric_parser_bounds_word_expansion_and_preserves_overlaps() {
    let many = format!(
        "[00:00]{}",
        (0..600)
            .map(|ms| format!("<00:00.{ms:03}>a"))
            .collect::<String>()
    );
    assert!(Lyrics::from_text(&many).is_ok());
    let overflow = std::iter::repeat_n(many.as_str(), 84)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(Lyrics::from_text(&overflow).is_err());
    let ttml = r#"<tt xmlns:itunes="http://music.apple.com/lyric-ttml-internal"><body><p begin="1s" end="5s"><span begin="1s" end="5s">first</span></p><p begin="3s" end="6s"><span begin="3s" end="6s">overlap</span></p></body></tt>"#;
    let cues = Lyrics::from_text(ttml).unwrap().display_cues();
    assert_eq!(cues[0].words[0].end, Some(5000));
    assert_eq!(cues[1].words[0].start, 3000);
}

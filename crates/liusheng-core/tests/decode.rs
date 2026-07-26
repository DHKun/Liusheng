mod common;

use liusheng_core::audio::decode::AudioFileDecoder;
use liusheng_core::audio::sink::{AudioSink, WavSink};

#[test]
fn decode_16bit_is_bit_exact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ramp16.wav");
    common::write_ramp_wav16(&path, 8000, 2000, 100);

    let mut dec = AudioFileDecoder::open(&path).unwrap();
    let spec = dec.spec();
    assert_eq!((spec.rate, spec.channels, spec.bits), (8000, 2, 16));
    assert_eq!(dec.duration_secs(), Some(0.25));

    let mut all: Vec<i32> = Vec::new();
    let mut buf = Vec::new();
    while dec.next_into(&mut buf).unwrap() {
        all.extend_from_slice(&buf);
    }
    assert_eq!(all.len(), 2000 * 2);
    // i16 -> i32 满量程为左移 16 位，逐比特可还原
    for (i, &s) in all.iter().enumerate() {
        let frame = i / 2;
        assert_eq!(s, (100 + frame as i32) << 16, "frame {frame}");
    }
}

#[test]
fn decode_24bit_is_bit_exact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ramp24.wav");
    common::write_ramp_wav24(&path, 8000, 1000, 4000);

    let mut dec = AudioFileDecoder::open(&path).unwrap();
    assert_eq!(dec.spec().bits, 24);

    let mut all: Vec<i32> = Vec::new();
    let mut buf = Vec::new();
    while dec.next_into(&mut buf).unwrap() {
        all.extend_from_slice(&buf);
    }
    assert_eq!(all.len(), 1000 * 2);
    for (i, &s) in all.iter().enumerate() {
        let frame = i / 2;
        assert_eq!(s, ((4000 + frame as i32) << 4) << 8, "frame {frame}");
    }
}

#[test]
fn seek_lands_on_exact_frame() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("seek.wav");
    common::write_ramp_wav16(&path, 8000, 8000, 0);

    let mut dec = AudioFileDecoder::open(&path).unwrap();
    let actual = dec.seek_secs(0.5).unwrap();
    let expected_frame = (actual * 8000.0).round() as i32;

    let mut buf = Vec::new();
    assert!(dec.next_into(&mut buf).unwrap());
    assert_eq!(buf[0], expected_frame << 16);
    // Accurate 模式不允许越过目标
    assert!(actual <= 0.5 + 1e-9, "actual = {actual}");
}

#[test]
fn wav_sink_roundtrip_preserves_source_bits() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.wav");
    let out = dir.path().join("out.wav");
    common::write_ramp_wav16(&src, 8000, 500, -200);

    let mut dec = AudioFileDecoder::open(&src).unwrap();
    let spec = dec.spec();
    let mut sink = WavSink::create(&out);
    let mut buf = Vec::new();
    while dec.next_into(&mut buf).unwrap() {
        sink.write(spec, &buf).unwrap();
    }
    sink.flush().unwrap();

    let mut reader = hound::WavReader::open(&out).unwrap();
    assert_eq!(reader.spec().bits_per_sample, 16);
    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    assert_eq!(samples.len(), 500 * 2);
    for (i, &s) in samples.iter().enumerate() {
        assert_eq!(s as i32, -200 + (i / 2) as i32);
    }
}

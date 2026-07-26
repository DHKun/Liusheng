use std::path::Path;
use std::time::Instant;

use liusheng_core::audio::decode::AudioFileDecoder;
use liusheng_core::audio::sink::{AudioSink, WavSink};
use liusheng_core::library::Library;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") => {
            let dir = args.get(1).map(String::as_str).unwrap_or("/data/Music");
            scan(Path::new(dir), None)
        }
        Some("search") => {
            let dir = args.get(1).map(String::as_str).unwrap_or("/data/Music");
            let query = args.get(2).cloned().unwrap_or_default();
            scan(Path::new(dir), Some(&query))
        }
        Some("decode") => {
            let input = args.get(1).expect("缺少输入文件");
            let output = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| "/tmp/liusheng-decode-test.wav".to_string());
            decode(Path::new(input), Path::new(&output))
        }
        _ => {
            eprintln!("用法:");
            eprintln!("  dev scan <目录>            扫描并列出曲库");
            eprintln!("  dev search <目录> <关键词>  扫描后搜索（支持拼音/首字母）");
            eprintln!("  dev decode <输入> [输出]    解码为 wav 验证");
            Ok(())
        }
    }
}

fn scan(dir: &Path, query: Option<&str>) -> anyhow::Result<()> {
    let db_path = std::env::temp_dir().join("liusheng-dev.db");
    let mut lib = Library::open(&db_path)?;
    let t = Instant::now();
    let stats = lib.scan(dir)?;
    println!(
        "扫描 {} 完成，耗时 {:?}：新增 {} 更新 {} 删除 {} 未变 {} 失败 {}",
        dir.display(),
        t.elapsed(),
        stats.added,
        stats.updated,
        stats.removed,
        stats.unchanged,
        stats.failed
    );
    let rows = match query {
        Some(q) if !q.is_empty() => {
            println!("搜索 \"{q}\":");
            lib.search(q, 20)?
        }
        _ => lib.all_tracks()?,
    };
    for r in rows.iter().take(20) {
        println!(
            "  {} - {} [{}] {}Hz/{}bit {}ms",
            r.artist,
            r.title,
            r.album,
            r.sample_rate,
            r.bit_depth
                .map(|b| b.to_string())
                .unwrap_or_else(|| "?".into()),
            r.duration_ms
        );
    }
    println!("共 {} 首", lib.track_count()?);
    Ok(())
}

fn decode(input: &Path, output: &Path) -> anyhow::Result<()> {
    let mut dec = AudioFileDecoder::open(input)?;
    let spec = dec.spec();
    println!(
        "{}: {}Hz {}ch {}bit，时长 {:?}s",
        input.display(),
        spec.rate,
        spec.channels,
        spec.bits,
        dec.duration_secs()
    );
    let mut sink = WavSink::create(output);
    let mut buf = Vec::new();
    let mut frames: u64 = 0;
    let t = Instant::now();
    while dec.next_into(&mut buf)? {
        sink.write(spec, &buf)?;
        frames += spec.frames(buf.len());
    }
    sink.flush()?;
    println!(
        "解码 {} 帧（{:.1}s 音频）耗时 {:?}，输出 {}",
        frames,
        frames as f64 / spec.rate as f64,
        t.elapsed(),
        output.display()
    );
    Ok(())
}

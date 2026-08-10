use std::path::{Path, PathBuf};
use std::time::Instant;

use liusheng_core::audio::PcmSpec;
use liusheng_core::audio::alsa_sink::AlsaSink;
use liusheng_core::audio::decode::AudioFileDecoder;
use liusheng_core::audio::pipewire_sink::PipeWireSink;
use liusheng_core::audio::sink::{AudioSink, WavSink};
use liusheng_core::engine::{Command, Player, PlayerEvent};
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
        Some("play") => {
            let paths: Vec<PathBuf> = args[1..].iter().map(PathBuf::from).collect();
            if paths.is_empty() {
                anyhow::bail!("缺少要播放的文件");
            }
            play(paths)
        }
        Some("alsa-probe") => {
            let device = args.get(1).map(String::as_str).unwrap_or("hw:Hybrid,0");
            alsa_probe(device)
        }
        _ => {
            eprintln!("用法:");
            eprintln!("  dev scan <目录>            扫描并列出曲库");
            eprintln!("  dev search <目录> <关键词>  扫描后搜索（支持拼音/首字母）");
            eprintln!("  dev decode <输入> [输出]    解码为 wav 验证");
            eprintln!("  dev play <文件>...          经 PipeWire 播放（Ctrl-C 退出）");
            eprintln!("  dev alsa-probe [设备]       用静音验证 ALSA 独占格式");
            Ok(())
        }
    }
}

fn alsa_probe(device: &str) -> anyhow::Result<()> {
    let mut sink = AlsaSink::new(device)?;
    println!("已独占打开 {}", sink.device());
    for (rate, bits) in [(48_000, 16), (48_000, 24), (96_000, 16), (96_000, 24)] {
        let spec = PcmSpec {
            rate,
            channels: 2,
            bits,
        };
        let silence = vec![0; rate as usize / 20 * usize::from(spec.channels)];
        sink.write(spec, &silence)?;
        sink.pause(true)?;
        sink.pause(false)?;
        sink.discard()?;
        println!("已写入 {rate} Hz / {bits} 位静音");
    }
    sink.flush()?;
    println!("ALSA 独占格式验证完成");
    Ok(())
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

fn play(paths: Vec<PathBuf>) -> anyhow::Result<()> {
    let sink = PipeWireSink::new()?;
    let player = Player::new(Box::new(sink));
    player.send(Command::SetQueue { paths, start: 0 });
    player.send(Command::Play);
    println!("控制：p 暂停/继续，n 下一曲，b 上一曲，s <秒> 跳转，q 退出（均需回车）");

    // stdin 独立线程转发到通道，主循环用 select 同时收事件与按键
    let (line_tx, line_rx) = crossbeam_channel::unbounded::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines() {
            let Ok(line) = line else { break };
            if line_tx.send(line).is_err() {
                break;
            }
        }
    });

    let mut paused = false;
    let mut line_rx = line_rx;
    loop {
        crossbeam_channel::select! {
            recv(player.events()) -> ev => {
                let Ok(ev) = ev else { break };
                match ev {
                    PlayerEvent::TrackStarted { index, path, spec, duration_secs } => {
                        println!(
                            "▶ [{}] {} — {}Hz/{}bit/{}ch，时长 {}",
                            index + 1,
                            path.display(),
                            spec.rate,
                            spec.bits,
                            spec.channels,
                            duration_secs
                                .map(|d| format!("{d:.1}s"))
                                .unwrap_or_else(|| "未知".into()),
                        );
                    }
                    PlayerEvent::Progress { secs } => {
                        print!("\r  {secs:>7.1}s");
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    }
                    PlayerEvent::Paused => {
                        paused = true;
                        println!("\n⏸ 已暂停");
                    }
                    PlayerEvent::Resumed => {
                        paused = false;
                        println!("\n▶ 继续");
                    }
                    PlayerEvent::TrackError { path, message } => {
                        eprintln!("\n跳过 {}: {}", path.display(), message);
                    }
                    PlayerEvent::EngineError { message } => {
                        eprintln!("\n引擎错误: {message}");
                    }
                    PlayerEvent::QueueFinished => {
                        println!("\n播放完毕");
                        break;
                    }
                    PlayerEvent::Stopped => {}
                }
            }
            recv(line_rx) -> line => {
                let Ok(line) = line else {
                    // stdin 关闭（重定向/后台运行），继续播放，仅停止收键
                    line_rx = crossbeam_channel::never();
                    continue;
                };
                let mut parts = line.split_whitespace();
                match parts.next() {
                    Some("p") => {
                        player.send(if paused { Command::Play } else { Command::Pause });
                    }
                    Some("n") => player.send(Command::Next),
                    Some("b") => player.send(Command::Prev),
                    Some("s") => match parts.next().and_then(|v| v.parse::<f64>().ok()) {
                        Some(secs) => player.send(Command::Seek(secs)),
                        None => eprintln!("用法: s <秒>"),
                    },
                    Some("q") => break,
                    _ => {}
                }
            }
        }
    }
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

use std::path::{Path, PathBuf};
use std::time::Instant;

#[cfg(target_os = "linux")]
use liusheng_core::audio::PcmSpec;
#[cfg(target_os = "linux")]
use liusheng_core::audio::alsa_sink::AlsaSink;
#[cfg(target_os = "macos")]
use liusheng_core::audio::coreaudio_sink::CoreAudioSink as NativeSink;
use liusheng_core::audio::decode::AudioFileDecoder;
#[cfg(target_os = "linux")]
use liusheng_core::audio::hardware_volume::{HardwareVolume, VolumeChange};
#[cfg(target_os = "linux")]
use liusheng_core::audio::pipewire_sink::PipeWireSink as NativeSink;
#[cfg(target_os = "linux")]
use liusheng_core::audio::resampling_sink::ResamplingSink;
use liusheng_core::audio::sink::{AudioSink, WavSink};
use liusheng_core::engine::{Player, PlayerCommand as Command, PlayerEvent};
use liusheng_core::library::Library;
use liusheng_core::settings::AppSettings;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args)
}

fn run(args: &[String]) -> anyhow::Result<()> {
    match args.first().map(String::as_str) {
        Some("scan") => {
            let dir = library_root(args.get(1));
            scan(&dir, None)
        }
        Some("search") => {
            let dir = library_root(args.get(1));
            let query = args.get(2).cloned().unwrap_or_default();
            scan(&dir, Some(&query))
        }
        Some("decode") => {
            let input = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("用法: dev decode <输入> [输出]"))?;
            let output = args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("liusheng-decode-test.wav"));
            decode(Path::new(input), &output)
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
        Some("volume-probe") => {
            let device = args.get(1).map(String::as_str).unwrap_or("hw:Hybrid");
            let element = args.get(2).map(String::as_str).unwrap_or("PCM");
            volume_probe(device, element)
        }
        None | Some("help" | "--help" | "-h") => {
            println!("{}", usage());
            Ok(())
        }
        Some(command) => anyhow::bail!("未知命令：{command}\n{}", usage()),
    }
}

fn library_root(explicit: Option<&String>) -> PathBuf {
    explicit.map(PathBuf::from).unwrap_or_else(|| {
        AppSettings::default()
            .music_roots
            .into_iter()
            .next()
            .expect("默认设置包含音乐目录")
    })
}

#[cfg(target_os = "linux")]
const PLAYBACK_BACKEND: &str = "PipeWire";
#[cfg(target_os = "macos")]
const PLAYBACK_BACKEND: &str = "CoreAudio";

fn usage() -> String {
    let mut help = format!(
        "用法:\n  dev scan [目录]             扫描并列出曲库\n  dev search <目录> <关键词>  扫描后搜索（支持拼音/首字母）\n  dev decode <输入> [输出]    解码为 wav 验证\n  dev play <文件>...          经 {PLAYBACK_BACKEND} 播放（q + 回车退出）\n"
    );
    if cfg!(target_os = "linux") {
        help.push_str("  dev alsa-probe [设备]       用静音验证 ALSA 独占格式\n  dev volume-probe [设备] [控件] 验证 ALSA 硬件音量控件\n");
    }
    help
}

#[cfg(not(target_os = "linux"))]
fn alsa_probe(_device: &str) -> anyhow::Result<()> {
    anyhow::bail!(
        "alsa-probe 仅支持 Linux ALSA；当前平台使用 {PLAYBACK_BACKEND}，请通过 dev play 验证播放"
    )
}

#[cfg(not(target_os = "linux"))]
fn volume_probe(_device: &str, _element: &str) -> anyhow::Result<()> {
    anyhow::bail!("volume-probe 仅支持 Linux ALSA；请使用系统或设备的音量控件")
}

#[cfg(target_os = "linux")]
fn volume_probe(device: &str, element: &str) -> anyhow::Result<()> {
    let volume = HardwareVolume::open(device, element)?;
    let initial = volume.state()?;
    let after_volume = volume.apply(VolumeChange::Percent(initial.percent))?;
    let final_state = if initial.can_mute {
        volume.apply(VolumeChange::Muted(initial.muted))?
    } else {
        after_volume
    };
    println!(
        "硬件音量验证完成：{}%，静音 {}，静音开关 {}",
        final_state.percent,
        if final_state.muted {
            "开启"
        } else {
            "关闭"
        },
        if final_state.can_mute {
            "可用"
        } else {
            "不可用"
        }
    );
    Ok(())
}

#[cfg(target_os = "linux")]
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

    let mut sink = ResamplingSink::new(Box::new(sink));
    let spec = PcmSpec {
        rate: 44_100,
        channels: 2,
        bits: 16,
    };
    let silence = vec![0; 44_100 / 5 * usize::from(spec.channels)];
    sink.write(spec, &silence)?;
    sink.flush()?;
    println!("已将 44.1 kHz / 16 位静音重采样到 96 kHz / 24 位");
    println!("ALSA 独占输出验证完成");
    Ok(())
}

fn scan(dir: &Path, query: Option<&str>) -> anyhow::Result<()> {
    // A private database keeps parallel CLI checks and user scans independent.
    let workspace = tempfile::tempdir()?;
    let mut lib = Library::open(&workspace.path().join("library.db"))?;
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
    let sink = NativeSink::new()?;
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
                    PlayerEvent::NavigationChanged(_) | PlayerEvent::Stopped | PlayerEvent::PreloadReady { .. } | PlayerEvent::OutputInfo { .. } => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn help_reports_the_native_backend_and_supported_probes() {
        let help = usage();
        #[cfg(target_os = "linux")]
        {
            assert!(help.contains("PipeWire"));
            assert!(help.contains("alsa-probe"));
            assert!(help.contains("volume-probe"));
        }
        #[cfg(target_os = "macos")]
        {
            assert!(help.contains("CoreAudio"));
            assert!(!help.contains("PipeWire"));
            assert!(!help.contains("alsa-probe"));
            assert!(!help.contains("volume-probe"));
        }
        for flags in [vec![], args(&["--help"]), args(&["help"])] {
            run(&flags).unwrap();
        }
    }

    #[test]
    fn invalid_arguments_return_errors_before_opening_audio() {
        for command in ["decode", "play", "unknown-command"] {
            assert!(run(&args(&[command])).is_err(), "{command}");
        }
    }

    #[test]
    fn library_directory_matches_application_defaults() {
        let defaults = AppSettings::default();
        assert_eq!(library_root(None), defaults.music_roots[0]);
        assert_eq!(
            library_root(Some(&"explicit music directory".into())),
            PathBuf::from("explicit music directory")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn linux_probes_report_platform_limitations_without_opening_devices() {
        assert!(
            run(&args(&["alsa-probe"]))
                .unwrap_err()
                .to_string()
                .contains("Linux ALSA")
        );
        assert!(
            run(&args(&["volume-probe"]))
                .unwrap_err()
                .to_string()
                .contains("Linux ALSA")
        );
    }

    #[test]
    fn decode_command_preserves_audio_samples_without_a_device() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("输入 with spaces.wav");
        let output = directory.path().join("decoded.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let samples = [-32_768i16, 32_767, -1234, 1234, 0, 42];
        let mut writer = hound::WavWriter::create(&input, spec).unwrap();
        for sample in samples {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
        run(&[
            "decode".into(),
            input.to_string_lossy().into_owned(),
            output.to_string_lossy().into_owned(),
        ])
        .unwrap();
        let mut reader = hound::WavReader::open(output).unwrap();
        assert_eq!(reader.spec(), spec);
        assert_eq!(
            reader
                .samples::<i16>()
                .map(Result::unwrap)
                .collect::<Vec<_>>(),
            samples
        );
    }

    #[test]
    fn scan_and_search_keep_their_database_isolated() {
        let directory = tempfile::tempdir().unwrap();
        let music = directory.path().join("音乐");
        std::fs::create_dir(&music).unwrap();
        let path = music.to_string_lossy().into_owned();
        run(&["scan".into(), path.clone()]).unwrap();
        run(&["search".into(), path, "测试".into()]).unwrap();
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(music).unwrap().count(), 0);
    }
}

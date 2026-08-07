use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use pipewire as pw;
use pw::spa;
use pw::spa::pod::Pod;

use crate::audio::PcmSpec;
use crate::audio::sink::AudioSink;
use crate::error::{Error, Result};

/// 环形缓冲的目标深度，按音频时间计。深度越大越抗断流，
/// 但 pause/discard 之外的尾音也越长，取 200ms 折中。
const BUFFER_DEPTH: Duration = Duration::from_millis(200);
/// 等待 PipeWire 线程就绪/排空的上限，超时视为输出端故障。
const WAIT_LIMIT: Duration = Duration::from_secs(10);

enum PwMsg {
    Connect(PcmSpec),
    SetActive(bool),
    Drain,
    Quit,
}

/// 当前活跃的流与其事件监听器，换格式时整体替换。
type ActiveStream = Option<(pw::stream::StreamRc, pw::stream::StreamListener<()>)>;

#[derive(Default)]
struct State {
    ring: VecDeque<i32>,
    cap: usize,
    ready: bool,
    drained: bool,
    error: Option<String>,
}

/// 引擎线程（生产者）与 PipeWire 线程（消费者）共享的状态。
/// cond 在腾出空间、排空完成、就绪、出错时广播。
struct Shared {
    state: Mutex<State>,
    cond: Condvar,
}

impl Shared {
    fn fail(&self, msg: String) {
        let mut st = self.state.lock().unwrap();
        st.error.get_or_insert(msg);
        self.cond.notify_all();
    }
}

/// PipeWire 原生客户端输出（共享模式）。
/// 流以来源采样率建节点，混音与设备适配交给 PipeWire 图。
/// 样本以 S32 全量程送出，16/24 位内容在 32 位容器中无损。
pub struct PipeWireSink {
    shared: Arc<Shared>,
    tx: pw::channel::Sender<PwMsg>,
    handle: Option<JoinHandle<()>>,
    spec: Option<PcmSpec>,
    paused: bool,
}

impl PipeWireSink {
    pub fn new() -> Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            cond: Condvar::new(),
        });
        let (tx, rx) = pw::channel::channel();
        let thread_shared = shared.clone();
        let handle = std::thread::Builder::new()
            .name("liusheng-pipewire".into())
            .spawn(move || pw_thread(thread_shared, rx))
            .map_err(|e| Error::Other(format!("无法创建 PipeWire 线程: {e}")))?;

        let sink = Self {
            shared,
            tx,
            handle: Some(handle),
            spec: None,
            paused: false,
        };
        // 等连上 PipeWire 守护进程，连不上在此就报错而非首次 write 才发现
        let st = sink.shared.state.lock().unwrap();
        let (st, _) = sink
            .shared
            .cond
            .wait_timeout_while(st, WAIT_LIMIT, |s| !s.ready && s.error.is_none())
            .unwrap();
        match &st.error {
            Some(e) => Err(Error::Other(e.clone())),
            None if !st.ready => Err(Error::Other("等待 PipeWire 连接超时".into())),
            None => {
                drop(st);
                Ok(sink)
            }
        }
    }

    fn send(&self, msg: PwMsg) -> Result<()> {
        self.tx
            .send(msg)
            .map_err(|_| Error::Other("PipeWire 线程已退出".into()))
    }

    fn check_error(st: &MutexGuard<'_, State>) -> Result<()> {
        match &st.error {
            Some(e) => Err(Error::Other(e.clone())),
            None => Ok(()),
        }
    }

    /// 等缓冲播空，再让流排空自身队列。
    fn drain(&mut self) -> Result<()> {
        let deadline = Instant::now() + WAIT_LIMIT;
        {
            let mut st = self.shared.state.lock().unwrap();
            while !st.ring.is_empty() {
                Self::check_error(&st)?;
                if Instant::now() >= deadline {
                    return Err(Error::Other("等待缓冲播空超时".into()));
                }
                let (g, _) = self
                    .shared
                    .cond
                    .wait_timeout(st, Duration::from_millis(200))
                    .unwrap();
                st = g;
            }
            st.drained = false;
        }
        self.send(PwMsg::Drain)?;
        let st = self.shared.state.lock().unwrap();
        let (st, timeout) = self
            .shared
            .cond
            .wait_timeout_while(st, WAIT_LIMIT, |s| !s.drained && s.error.is_none())
            .unwrap();
        Self::check_error(&st)?;
        if timeout.timed_out() {
            return Err(Error::Other("等待流排空超时".into()));
        }
        Ok(())
    }
}

impl AudioSink for PipeWireSink {
    fn write(&mut self, spec: PcmSpec, samples: &[i32]) -> Result<()> {
        if self.spec != Some(spec) {
            // 换格式先把旧流播完再重建，衔接处不丢尾音
            if self.spec.is_some() && !self.paused {
                self.drain()?;
            }
            let cap = (spec.rate as usize * spec.channels as usize)
                .saturating_mul(BUFFER_DEPTH.as_millis() as usize)
                / 1000;
            {
                let mut st = self.shared.state.lock().unwrap();
                st.cap = cap.max(samples.len());
                st.ring.clear();
            }
            self.send(PwMsg::Connect(spec))?;
            self.spec = Some(spec);
        }
        let mut offset = 0;
        let mut st = self.shared.state.lock().unwrap();
        while offset < samples.len() {
            Self::check_error(&st)?;
            let space = st.cap.saturating_sub(st.ring.len());
            if space == 0 {
                let (g, _) = self
                    .shared
                    .cond
                    .wait_timeout(st, Duration::from_millis(200))
                    .unwrap();
                st = g;
                continue;
            }
            let n = space.min(samples.len() - offset);
            st.ring.extend(&samples[offset..offset + n]);
            offset += n;
        }
        Ok(())
    }

    fn pause(&mut self, paused: bool) -> Result<()> {
        if self.paused == paused {
            return Ok(());
        }
        self.paused = paused;
        if self.spec.is_some() {
            self.send(PwMsg::SetActive(!paused))?;
        }
        Ok(())
    }

    fn discard(&mut self) -> Result<()> {
        let mut st = self.shared.state.lock().unwrap();
        st.ring.clear();
        drop(st);
        self.shared.cond.notify_all();
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.spec.is_none() {
            return Ok(());
        }
        if self.paused {
            // 暂停中无法排空（流不消耗），只发生在退出路径，直接丢弃
            self.discard()?;
        } else {
            self.drain()?;
        }
        // 置空让下次 write 重建流，绕开已排空流的复活语义差异
        self.spec = None;
        Ok(())
    }
}

impl Drop for PipeWireSink {
    fn drop(&mut self) {
        let _ = self.tx.send(PwMsg::Quit);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn pw_thread(shared: Arc<Shared>, rx: pw::channel::Receiver<PwMsg>) {
    if let Err(e) = run_loop(&shared, rx) {
        shared.fail(format!("PipeWire 连接失败: {e}"));
    }
}

fn run_loop(
    shared: &Arc<Shared>,
    rx: pw::channel::Receiver<PwMsg>,
) -> std::result::Result<(), pw::Error> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;

    {
        let mut st = shared.state.lock().unwrap();
        st.ready = true;
    }
    shared.cond.notify_all();

    let current: std::rc::Rc<std::cell::RefCell<ActiveStream>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));

    let _rx = rx.attach(mainloop.loop_(), {
        let mainloop = mainloop.clone();
        let shared = shared.clone();
        let current = current.clone();
        move |msg| match msg {
            PwMsg::Connect(spec) => {
                if let Some((stream, _listener)) = current.borrow_mut().take() {
                    let _ = stream.disconnect();
                }
                match build_stream(core.clone(), spec, &shared) {
                    Ok(pair) => *current.borrow_mut() = Some(pair),
                    Err(e) => shared.fail(format!("创建 PipeWire 流失败: {e}")),
                }
            }
            PwMsg::SetActive(active) => {
                if let Some((stream, _)) = &*current.borrow() {
                    let _ = stream.set_active(active);
                }
            }
            PwMsg::Drain => {
                let borrowed = current.borrow();
                match &*borrowed {
                    Some((stream, _)) => {
                        let _ = stream.flush(true);
                    }
                    None => {
                        // 没有流可排空，直接视为完成
                        let mut st = shared.state.lock().unwrap();
                        st.drained = true;
                        drop(st);
                        shared.cond.notify_all();
                    }
                }
            }
            PwMsg::Quit => mainloop.quit(),
        }
    });

    mainloop.run();
    Ok(())
}

fn build_stream(
    core: pw::core::CoreRc,
    spec: PcmSpec,
    shared: &Arc<Shared>,
) -> std::result::Result<(pw::stream::StreamRc, pw::stream::StreamListener<()>), pw::Error> {
    let stream = pw::stream::StreamRc::new(
        core,
        "Liusheng",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Playback",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::APP_NAME => "Liusheng",
            *pw::keys::APP_ID => "io.github.dhkun.Liusheng",
            *pw::keys::NODE_NAME => "Liusheng",
        },
    )?;

    let channels = spec.channels.max(1) as usize;
    let listener = stream
        .add_local_listener::<()>()
        .process({
            let shared = shared.clone();
            move |stream, _| process(stream, &shared, channels)
        })
        .drained({
            let shared = shared.clone();
            move |_, _| {
                let mut st = shared.state.lock().unwrap();
                st.drained = true;
                drop(st);
                shared.cond.notify_all();
            }
        })
        .state_changed({
            let shared = shared.clone();
            move |_, _, _, new| {
                if let pw::stream::StreamState::Error(e) = new {
                    shared.fail(format!("PipeWire 流错误: {e}"));
                }
            }
        })
        .register()?;

    let mut info = spa::param::audio::AudioInfoRaw::new();
    info.set_format(spa::param::audio::AudioFormat::S32LE);
    info.set_rate(spec.rate);
    info.set_channels(channels as u32);
    let mut position = [0u32; spa::param::audio::MAX_CHANNELS];
    match channels {
        1 => {
            position[0] = spa::sys::SPA_AUDIO_CHANNEL_MONO;
            info.set_position(position);
        }
        2 => {
            position[0] = spa::sys::SPA_AUDIO_CHANNEL_FL;
            position[1] = spa::sys::SPA_AUDIO_CHANNEL_FR;
            info.set_position(position);
        }
        // 其他声道数不指定布局，交给 PipeWire 按 UNPOSITIONED 处理
        _ => {}
    }

    let values: Vec<u8> = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(spa::pod::Object {
            type_: spa::sys::SPA_TYPE_OBJECT_Format,
            id: spa::sys::SPA_PARAM_EnumFormat,
            properties: info.into(),
        }),
    )
    .expect("音频格式 Pod 序列化失败")
    .0
    .into_inner();
    let mut params = [Pod::from_bytes(&values).expect("音频格式 Pod 解析失败")];

    stream.connect(
        spa::utils::Direction::Output,
        None,
        pw::stream::StreamFlags::AUTOCONNECT
            | pw::stream::StreamFlags::MAP_BUFFERS
            | pw::stream::StreamFlags::RT_PROCESS,
        &mut params,
    )?;

    Ok((stream, listener))
}

/// 实时回调：从环形缓冲取样本填充输出缓冲。缓冲不足时送短帧，不足处为静音。
fn process(stream: &pw::stream::Stream, shared: &Shared, channels: usize) {
    let Some(mut buffer) = stream.dequeue_buffer() else {
        return;
    };
    let requested = buffer.requested() as usize;
    let datas = buffer.datas_mut();
    let Some(data) = datas.first_mut() else {
        return;
    };
    let stride = 4 * channels;
    let mut n_frames = 0;
    if let Some(slice) = data.data() {
        let max_frames = slice.len() / stride;
        let want = if requested > 0 {
            requested.min(max_frames)
        } else {
            max_frames
        };
        let mut st = shared.state.lock().unwrap();
        n_frames = want.min(st.ring.len() / channels);
        for (i, s) in st.ring.drain(..n_frames * channels).enumerate() {
            slice[i * 4..i * 4 + 4].copy_from_slice(&s.to_le_bytes());
        }
        drop(st);
        shared.cond.notify_all();
    }
    let chunk = data.chunk_mut();
    *chunk.offset_mut() = 0;
    *chunk.stride_mut() = stride as i32;
    *chunk.size_mut() = (n_frames * stride) as u32;
}

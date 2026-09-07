use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use liusheng_core::audio::hardware_volume::{HardwareVolume, VolumeChange, VolumeState};
pub enum VolumeCommand {
    Refresh(String, String),
    Change(VolumeChange),
    Quit,
}
pub struct VolumeService {
    commands: Sender<VolumeCommand>,
    events: Receiver<Result<VolumeState, String>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl VolumeService {
    pub fn start() -> std::io::Result<Self> {
        let (commands, rx) = bounded(16);
        let (tx, events) = unbounded();
        let worker = std::thread::Builder::new()
            .name("liusheng-volume".into())
            .spawn(move || {
                let mut current = None::<(String, String, HardwareVolume)>;
                while let Ok(command) = rx.recv() {
                    let result = match command {
                        VolumeCommand::Quit => break,
                        VolumeCommand::Refresh(device, element) => {
                            if current
                                .as_ref()
                                .is_none_or(|(d, e, _)| *d != device || *e != element)
                            {
                                match HardwareVolume::open(&device, &element) {
                                    Ok(v) => current = Some((device, element, v)),
                                    Err(e) => {
                                        tx.send(Err(e.to_string())).ok();
                                        continue;
                                    }
                                }
                            }
                            current.as_ref().unwrap().2.state()
                        }
                        VolumeCommand::Change(change) => {
                            let Some((_, _, v)) = &current else { continue };
                            v.apply(change)
                        }
                    }
                    .map_err(|e| e.to_string());
                    if result.is_err() {
                        current = None;
                    }
                    if tx.send(result).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            commands,
            events,
            worker: Some(worker),
        })
    }
    pub fn send(&self, command: VolumeCommand) {
        let _ = self.commands.try_send(command);
    }
    pub fn events(&self) -> Receiver<Result<VolumeState, String>> {
        self.events.clone()
    }
}
impl Drop for VolumeService {
    fn drop(&mut self) {
        let _ = self.commands.send(VolumeCommand::Quit);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

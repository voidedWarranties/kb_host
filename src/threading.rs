use crate::{
    config::KBConfig,
    protocol::{ProtocolMessage, RgbSetFullMessage, RgbSetMessage, RAW_EPSIZE},
};
use crossbeam::channel::{unbounded, Receiver, Sender};
use hidapi::HidDevice;
use palette::Hsv;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Default, Clone)]
pub struct KeyState {
    pub last_down: Option<Instant>,
    pub is_down: bool,
}

#[derive(Default, Clone)]
pub struct HIDThreadState {
    pub delta_update: f32,
    pub delta_frame: f32,
    pub matrix: Vec<Vec<KeyState>>,
}

pub struct HIDThread {
    tx: Sender<HIDThreadState>,
    rx: Receiver<HIDThreadState>,
    cancel: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    kb_config: Arc<KBConfig>,
}

impl HIDThread {
    pub fn new(kb_config: Arc<KBConfig>) -> HIDThread {
        let (tx, rx) = unbounded::<HIDThreadState>();

        HIDThread {
            tx,
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            thread: None,
            kb_config,
        }
    }

    pub fn start(&mut self, update_rate: f32, frame_rate: f32, device: HidDevice) {
        let delta_update = 1.0 / update_rate;
        let delta_frame = 1.0 / frame_rate;
        let kb_config = self.kb_config.clone();
        let tx = self.tx.clone();
        let cancel_arc = self.cancel.clone();

        self.thread = Some(thread::spawn(move || {
            Self::run(delta_update, delta_frame, device, kb_config, tx, cancel_arc)
        }));
    }

    pub fn stop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.thread
            .take()
            .expect("thread has not started!")
            .join()
            .unwrap();
    }

    pub fn rx(&self) -> Receiver<HIDThreadState> {
        self.rx.clone()
    }

    fn run(
        wait_update: f32,
        wait_frame: f32,
        device: HidDevice,
        kb_config: Arc<KBConfig>,
        state_tx: Sender<HIDThreadState>,
        cancel: Arc<AtomicBool>,
    ) {
        let mut last_update = Instant::now();
        let mut last_frame = Instant::now();

        let mut delta_frame = wait_frame;

        let mut recv_buffer = [0u8; RAW_EPSIZE];

        let layout = kb_config.layout();

        let mut matrix = vec![
            vec![KeyState::default(); kb_config.columns() as usize];
            kb_config.rows() as usize
        ];

        let mut hue: f32 = 0.0;

        while !cancel.load(Ordering::Relaxed) {
            // prep
            let delta_update = last_update.elapsed().as_secs_f32();

            // work
            if let Ok(size) = device.read_timeout(&mut recv_buffer, 0) {
                match ProtocolMessage::read_buffer(&recv_buffer, size) {
                    Some(ProtocolMessage::Press(press)) => {
                        let key_state = &mut matrix[press.row as usize][press.col as usize];
                        key_state.is_down = press.pressed;

                        if press.pressed {
                            key_state.last_down = Some(Instant::now());
                        }
                    }
                    Some(ProtocolMessage::Layer(layer)) => {
                        dbg!(layer);
                    }
                    _ => {}
                }
            }

            if last_frame.elapsed() >= Duration::from_secs_f32(wait_frame) {
                delta_frame = last_frame.elapsed().as_secs_f32();

                let mut colors: HashMap<u8, Hsv> = HashMap::new();

                for key in &layout.layout {
                    let idx = kb_config.matrix[key.matrix.0 as usize][key.matrix.1 as usize];
                    if idx < -1 {
                        continue;
                    }

                    let mut key_hue = hue + (key.x + key.y) * 4.0;
                    key_hue %= 360.0;

                    colors.insert(idx as u8, Hsv::new(key_hue, 1.0, 1.0));
                }

                hue += 36.0 * delta_frame;
                hue %= 360.0;

                for chunk in colors.into_iter().collect::<Vec<_>>().chunks(7) {
                    let colors: HashMap<u8, Hsv> = chunk.iter().copied().collect();

                    ProtocolMessage::RgbSet(RgbSetMessage { colors })
                        .send(&device)
                        .ok();
                }

                last_frame = Instant::now();
            }

            // tx
            state_tx
                .try_send(HIDThreadState {
                    delta_update,
                    delta_frame,
                    matrix: matrix.clone(),
                })
                .ok();

            // sleep
            last_update = Instant::now();
            thread::sleep(Duration::from_secs_f32(wait_update));
        }

        ProtocolMessage::RgbSetFull(RgbSetFullMessage {
            color: Hsv::new(0.0, 0.0, 0.0),
        })
        .send(&device)
        .expect("failed to clear keyboard leds");
    }
}

impl Drop for HIDThread {
    fn drop(&mut self) {
        self.stop();
    }
}

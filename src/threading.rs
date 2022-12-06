use crate::{
    config::KBConfig,
    effects::{LedEffect, LedState},
    protocol::{ProtocolMessage, RgbSetFullMessage, RgbSetMessage, RAW_EPSIZE},
};
use crossbeam::channel::{unbounded, Receiver, Sender};
use hidapi::HidDevice;
use palette::{Hsv, Hsva};
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
    // last time the down event was sent for this key
    pub last_down: Option<Instant>,
    // last time this key was not up
    pub last_pressed: Option<Instant>,
    pub is_pressed: bool,
}

#[derive(Default, Clone)]
pub struct HIDThreadState {
    pub delta_update: f32,
    pub delta_frame: f32,
    pub matrix: Vec<Vec<KeyState>>,
    pub led_state: Vec<Hsva>,
    pub layer_state: u8,
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

        let mut effects: Vec<Box<dyn LedEffect>> =
            vec![Box::new(crate::effects::Rainbow1Effect::default())];

        let mut matrix = vec![
            vec![KeyState::default(); kb_config.columns() as usize];
            kb_config.rows() as usize
        ];

        let mut led_state = vec![LedState::default(); kb_config.led_count().into()];

        for key in &layout.layout {
            let led_idx = kb_config.matrix[key.matrix.0 as usize][key.matrix.1 as usize];
            if led_idx < 0 {
                continue;
            }

            led_state[led_idx as usize].key = Some(key);
        }

        let mut layer_state: u8 = 0;

        while !cancel.load(Ordering::Relaxed) {
            // prep
            let delta_update = last_update.elapsed().as_secs_f32();

            // work
            if let Ok(size) = device.read_timeout(&mut recv_buffer, 0) {
                match ProtocolMessage::read_buffer(&recv_buffer, size) {
                    Some(ProtocolMessage::Press(press)) => {
                        let key_state = &mut matrix[press.row as usize][press.col as usize];

                        if press.pressed {
                            if !key_state.is_pressed {
                                key_state.last_down = Some(Instant::now());
                            }

                            key_state.last_pressed = Some(Instant::now());
                        }

                        key_state.is_pressed = press.pressed;
                    }
                    Some(ProtocolMessage::Layer(layer)) => {
                        layer_state = layer.layer_state;
                    }
                    _ => {}
                }
            }

            if last_frame.elapsed() >= Duration::from_secs_f32(wait_frame) {
                delta_frame = last_frame.elapsed().as_secs_f32();

                let pre_state = led_state.clone();

                for effect in &mut effects {
                    effect.update(delta_frame, &mut led_state, &matrix);
                }

                let mut colors: HashMap<u8, Hsv> = HashMap::new();

                for (idx, led) in led_state.iter().enumerate() {
                    if led.color != pre_state[idx].color {
                        colors.insert(
                            idx as u8,
                            Hsv::new(
                                led.color.hue,
                                led.color.saturation,
                                led.color.value * led.color.alpha,
                            ),
                        );
                    }
                }

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
                    led_state: led_state.iter().map(|state| state.color).collect(),
                    layer_state,
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

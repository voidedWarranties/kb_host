use hidapi::HidApi;
use std::{fs, io, sync::Arc};

mod config;
use config::*;

mod threading;
use threading::HIDThread;

mod ui;

mod protocol;

mod effects;

const CONFIG_PATH: &str = "kb_host/config.json";
const UPDATE_RATE: f32 = 240.0; // <5 ms per update
const FPS: f32 = 20.0;

fn read_config() -> Result<KBConfig, io::Error> {
    // host configuration
    let config_contents = fs::read_to_string(
        dirs::config_dir()
            .expect("no config directory")
            .join(CONFIG_PATH),
    )?;
    let config: Config = serde_json::from_str(&config_contents)?;

    // info.json and matrix.json
    let keyboard_path = dirs::home_dir()
        .expect("no home directory")
        .join("qmk_firmware/keyboards")
        .join(&config.kb);

    let qmk_info_contents = fs::read_to_string(keyboard_path.join("info.json"))?;
    let qmk_info: QMKInfo = serde_json::from_str(&qmk_info_contents)?;

    let matrix_contents = fs::read_to_string(keyboard_path.join("matrix.json"))?;
    let matrix: LEDMatrix = serde_json::from_str(&matrix_contents)?;

    // legends.json
    let keymap_path = keyboard_path.join("keymaps").join(&config.keymap);

    let legends_contents = fs::read_to_string(keymap_path.join("legends.json"))?;
    let legends: KBLegends = serde_json::from_str(&legends_contents)?;

    Ok(KBConfig::new(config, qmk_info, matrix, legends))
}

fn main() -> Result<(), io::Error> {
    let kb_config = Arc::new(read_config()?);

    let device = match HidApi::new() {
        Ok(api) => {
            let device_info = api
                .device_list()
                .find(|device| {
                    device.vendor_id() == kb_config.qmk_info.usb.vid
                        && device.product_id() == kb_config.qmk_info.usb.pid
                        && device.usage_page() == kb_config.host_config.usage_page
                        && device.usage() == kb_config.host_config.usage
                })
                .expect("could not find device");

            api.open_path(device_info.path())
                .expect("could not open device")
        }
        Err(err) => panic!("could not list hid devices: {}", err),
    };

    // thread
    let mut thread = HIDThread::new(kb_config.clone());
    thread.start(UPDATE_RATE, FPS, device);

    // egui
    let options = eframe::NativeOptions {
        maximized: true,
        ..Default::default()
    };

    let rx = thread.rx();

    eframe::run_native(
        "ksk QMK keyboard host",
        options,
        Box::new(move |_cc| Box::new(ui::App::new(rx, kb_config))),
    );

    Ok(())
}

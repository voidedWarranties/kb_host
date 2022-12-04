use serde::{de::Visitor, Deserialize, Deserializer};
use std::collections::HashMap;

struct HexString;

impl<'de> Visitor<'de> for HexString {
    type Value = u16;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a hex string, optionally beginning with 0x")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        u16::from_str_radix(v.trim_start_matches("0x"), 16).map_err(serde::de::Error::custom)
    }
}

fn deserialize_hex<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(HexString)
}

pub struct KBConfig {
    pub host_config: Config,
    pub qmk_info: QMKInfo,
    pub matrix: LEDMatrix,
    width: f32,
    height: f32,
    rows: u8,
    columns: u8,
    led_count: u8,
}

impl KBConfig {
    pub fn new(host_config: Config, qmk_info: QMKInfo, matrix: LEDMatrix) -> KBConfig {
        let layout = Self::get_layout(&qmk_info, &host_config);

        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;

        let mut rows: u8 = 0;
        let mut columns: u8 = 0;

        let mut led_count: u8 = 0;

        for key in &layout.layout {
            width = width.max(key.x + key.w);
            height = height.max(key.y + key.h);

            rows = rows.max(key.matrix.0 + 1);
            columns = columns.max(key.matrix.1 + 1);

            if matrix[key.matrix.0 as usize][key.matrix.1 as usize] >= 0 {
                led_count += 1;
            }
        }

        KBConfig {
            host_config,
            qmk_info,
            matrix,
            width,
            height,
            rows,
            columns,
            led_count,
        }
    }

    fn get_layout<'a>(qmk_info: &'a QMKInfo, host_config: &'a Config) -> &'a QMKLayout {
        qmk_info
            .layouts
            .get(&host_config.layout)
            .expect("could not find layout")
    }

    pub fn layout(&self) -> &QMKLayout {
        Self::get_layout(&self.qmk_info, &self.host_config)
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn rows(&self) -> u8 {
        self.rows
    }

    pub fn columns(&self) -> u8 {
        self.columns
    }

    pub fn led_count(&self) -> u8 {
        self.led_count
    }
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub kb: String,
    pub layout: String,
    #[serde(deserialize_with = "deserialize_hex")]
    pub usage_page: u16,
    #[serde(deserialize_with = "deserialize_hex")]
    pub usage: u16,
}

#[derive(Deserialize, Debug)]
pub struct QMKInfo {
    pub keyboard_name: String,
    pub manufacturer: String,
    pub usb: QMKUSBInfo,
    pub layouts: HashMap<String, QMKLayout>,
}

#[derive(Deserialize, Debug)]
pub struct QMKUSBInfo {
    #[serde(deserialize_with = "deserialize_hex")]
    pub vid: u16,
    #[serde(deserialize_with = "deserialize_hex")]
    pub pid: u16,
    pub device_version: String,
}

#[derive(Deserialize, Debug)]
pub struct QMKLayout {
    pub layout: Vec<QMKKey>,
}

fn default_dim() -> f32 {
    1.0
}

#[derive(Deserialize, Debug)]
pub struct QMKKey {
    pub label: String,
    pub x: f32,
    pub y: f32,
    #[serde(default = "default_dim")]
    pub w: f32,
    #[serde(default = "default_dim")]
    pub h: f32,
    pub matrix: (u8, u8),
}

pub type LEDMatrix = Vec<Vec<i16>>;

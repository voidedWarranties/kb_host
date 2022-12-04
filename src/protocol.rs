use hidapi::{HidDevice, HidError};
use palette::{rgb::Rgb, Hsv, IntoColor};
use std::collections::HashMap;

pub const KSK_PRESS: u8 = 0;
pub const KSK_LAYER: u8 = 1;
pub const KSK_RGB_SET: u8 = 2;

pub const RAW_EPSIZE: usize = 32;

#[derive(Debug)]
pub struct PressMessage {
    pub pressed: bool,
    pub keycode: u16,
    pub col: u8,
    pub row: u8,
}

#[derive(Debug)]
pub struct LayerMessage {
    pub layer_state: u8,
}

#[derive(Debug)]
pub struct RgbSetMessage {
    pub colors: HashMap<u8, Hsv>,
}

#[derive(Debug)]
pub struct RgbSetFullMessage {
    pub color: Hsv,
}

#[derive(Debug)]
pub enum ProtocolMessage {
    Press(PressMessage),
    Layer(LayerMessage),
    RgbSet(RgbSetMessage),
    RgbSetFull(RgbSetFullMessage),
}

const K: u8 = 0x6b;
const S: u8 = 0x73;

fn push_color(buf: &mut Vec<u8>, color: &Hsv) {
    let rgb: Rgb = (*color).into_color();
    buf.push((rgb.red * 255.0) as u8);
    buf.push((rgb.green * 255.0) as u8);
    buf.push((rgb.blue * 255.0) as u8);
}

fn read_u16(buf: &[u8], beg_index: usize) -> u16 {
    (buf[beg_index + 1] as u16) << 4 | buf[beg_index] as u16
}

impl ProtocolMessage {
    pub fn read_buffer(buf: &[u8; RAW_EPSIZE], size: usize) -> Option<ProtocolMessage> {
        if size < 4 || buf[0] != K || buf[1] != S || buf[2] != K {
            return None;
        }

        let op = buf[3] >> 4;
        let header_data = buf[3] & 0b00001111;

        match op {
            KSK_PRESS => Some(ProtocolMessage::Press(PressMessage {
                pressed: header_data == 1,
                keycode: read_u16(buf, 4),
                col: buf[6],
                row: buf[7],
            })),
            KSK_LAYER => Some(ProtocolMessage::Layer(LayerMessage {
                layer_state: buf[4],
            })),
            _ => None,
        }
    }

    pub fn send(&self, device: &HidDevice) -> Result<usize, HidError> {
        let mut buf: Vec<u8> = vec![0x00, K, S, K];

        match self {
            ProtocolMessage::RgbSet(msg) => {
                if msg.colors.len() > u8::MAX.into() {
                    panic!("cannot set this many rgb pixels at once!");
                }

                buf.push(KSK_RGB_SET << 4 | msg.colors.len() as u8);

                for (idx, color) in &msg.colors {
                    buf.push(*idx);
                    push_color(&mut buf, color);
                }
            }
            ProtocolMessage::RgbSetFull(msg) => {
                buf.push(KSK_RGB_SET << 4);
                push_color(&mut buf, &msg.color);
            }
            _ => panic!("this message cannot be sent!"),
        }

        if buf.len() > RAW_EPSIZE + 1 {
            panic!("message size exceeded RAW_EPSIZE ({})", RAW_EPSIZE);
        }

        device.write(&buf)
    }
}

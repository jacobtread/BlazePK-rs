use std::net::TcpStream;
use crate::tdf::Tdf;

#[repr(u16)]
enum PacketType {
    Request = 0x0000,
    Response = 0x1000,
    Notify = 0x2000,
    Error = 0x3000,
}

pub struct Packet {
    component: u16,
    command: u16,
    error: u16,
    mode: PacketType,
    id: u16,
    contents: Vec<Tdf>,
}

impl Packet {
    pub fn response(packet: &Packet, contents: Vec<Tdf>) -> Self {
        Self {
            component: packet.component,
            command: packet.command,
            error: 0,
            mode: PacketType::Response,
            id: packet.id,
            contents,
        }
    }

    pub fn error(packet: &Packet, error: u16, contents: Vec<Tdf>) -> Self {
        Self {
            component: packet.component,
            command: packet.command,
            error,
            mode: PacketType::Response,
            id: packet.id,
            contents,
        }
    }

    pub fn notify(component: u16, command: u16, contents: Vec<Tdf>) -> Self {
        Self {
            component,
            command,
            error: 0,
            mode: PacketType::Notify,
            id: 0,
            contents,
        }
    }

    pub fn push(&mut self, value: Tdf) {
        self.contents.push(value);
    }
}


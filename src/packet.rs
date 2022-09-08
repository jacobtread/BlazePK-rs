use std::net::TcpStream;
use crate::tdf::Tdf;

pub struct Packet {
    component: u16,
    command: u16,
    error: u16,
    p_type: u16,
    id: u16,
    contents: Vec<Tdf>
}




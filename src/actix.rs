use crate::packet::{Packet, PacketHeader};
use actix_codec::{Decoder, Encoder};

use bytes::BytesMut;
use tokio::io;

pub struct PacketCodec;

impl Decoder for PacketCodec {
    type Item = Packet;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let (header, length) = match PacketHeader::read_bytes_mut(src) {
            Some(value) => value,
            None => return Ok(None),
        };
        if src.len() < length {
            return Ok(None);
        }
        let contents = src.split_off(length);
        Ok(Some(Packet {
            header,
            contents: contents.freeze(),
        }))
    }
}

impl Encoder<Packet> for PacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let contents = item.contents;
        item.header.write_bytes_mut(dst, contents.len());
        dst.extend_from_slice(&contents);
        Ok(())
    }
}

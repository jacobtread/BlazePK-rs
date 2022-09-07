use std::collections::HashMap;
use std::os::raw::c_ushort;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}


pub struct Packet {
    component: u16,
    command: u16,
    error: u16,
    p_type: u16,
    id: u16,
    contents: Vec<Tdf>
}

pub struct Tdf(String, TdfType);

type VarInt = u64;

pub enum TdfType {
    VarInt,
    String,
    Blob,
    Group,
    List,
    Map,
    Optional,
    IntList,
    Pair,
    Tripple,
    Float
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

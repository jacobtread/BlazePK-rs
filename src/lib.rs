pub mod packet;
pub mod tdf;
pub mod error;
pub mod io;

use std::collections::HashMap;
use std::os::raw::c_ushort;

pub fn add(left: usize, right: usize) -> usize {
    left + right
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

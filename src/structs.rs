use crate::codec::{Codec, Reader};
use crate::tdf::{Tag, ValueType};
use crate::types::{TdfMap, TdfOptional, VarInt, VarIntList};

/// Macro for generating structures that can be encoded and decoded from bytes
/// (DONT USE THIS FOR GROUPS USE `tdf_group` because they require extra bytes)
#[macro_export]
macro_rules! packet {
    (
        struct $name:ident {
            $(
                $field:ident: $ty:ty
            ),* $(,)?
        }

    ) => {
        #[derive(Debug)]
        #[allow(non_snake_case)]
        pub struct $name {
            $($field: $ty),*
        }

        impl $crate::codec::Codec for $name {

            #[allow(non_snake_case)]
            fn encode(&self, output: &mut Vec<u8>) {
                $(
                    $crate::tdf::Tag::encode_from(stringify!($field), &(<$ty>::value_type()), output);
                    <$ty>::encode(&self.$field, output);
                )*
            }

            #[allow(non_snake_case)]
            fn decode(reader: &mut $crate::codec::Reader) -> $crate::codec::CodecResult<Self>  {
                $(
                    println!("Read: {}", stringify!($field));
                    $crate::tdf::Tag::expect_tag(stringify!($field), &(<$ty>::value_type()), reader)?;
                    let $field = <$ty>::decode(reader)
                        .map_err(|err|$crate::codec::CodecError::DecodeFail(stringify!($field), Box::new(err)))?;
                    println!("Read Done: {:?} Cursor: {}", $field, reader.cursor());
                )*
                Ok(Self {
                    $($field),*
                })
            }
        }
    };
}

/// Macro for generating group structures prefixing the struct with (2)
/// indicates that when encoding a byte value of two should be placed
/// at the start.
#[macro_export]
macro_rules! group {
    (
        struct $name:ident {
            $(
                $field:ident: $ty:ty
            ),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        #[allow(non_snake_case)]
        pub struct $name {
            $($field: $ty),*
        }

        impl $crate::codec::Codec for $name {

            #[allow(non_snake_case)]
            fn encode(&self, output: &mut Vec<u8>) {
                $(
                    $crate::tdf::Tag::encode_from(stringify!($field), &(<$ty>::value_type()), output);
                    <$ty>::encode(&self.$field, output);
                )*
                output.push(0)
            }

            #[allow(non_snake_case)]
            fn decode(reader: &mut $crate::codec::Reader) -> $crate::codec::CodecResult<Self> {
                $crate::tdf::Tag::take_two(reader)?;
                $(
                    $crate::tdf::Tag::expect_tag(stringify!($field), &(<$ty>::value_type()), reader)?;
                    let $field = <$ty>::decode(reader)
                        .map_err(|err|$crate::codec::CodecError::DecodeFail(stringify!($field), Box::new(err)))?;
                )*
                $crate::tdf::Tag::discard_group(reader)?;
                Ok(Self {
                    $($field),*
                })
            }

            fn value_type() -> $crate::tdf::ValueType {
                $crate::tdf::ValueType::Group
            }
        }
    };
    (
        (2) struct $name:ident {
            $(
                $field:ident: $ty:ty
            ),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        #[allow(non_snake_case)]
        pub struct $name {
            $($field: $ty),*
        }

        impl $crate::codec::Codec for $name {

            #[allow(non_snake_case)]
            fn encode(&self, output: &mut Vec<u8>) {
                output.push(2);
                $(
                    $crate::tdf::Tag::encode_from(stringify!($field), &(<$ty>::value_type()), output);
                    <$ty>::encode(&self.$field, output);
                )*
                output.push(0);
            }

            #[allow(non_snake_case)]
            fn decode(reader: &mut $crate::codec::Reader) -> $crate::codec::CodecResult<Self> {
                $crate::tdf::Tag::take_two(reader)?;
                $(
                    $crate::tdf::Tag::expect_tag(stringify!($field), &(<$ty>::value_type()), reader)?;
                    let $field = <$ty>::decode(reader)
                        .map_err(|err|$crate::codec::CodecError::DecodeFail(stringify!($field), Box::new(err)))?;
                )*
                $crate::tdf::Tag::discard_group(reader)?;
                Ok(Self {
                    $($field),*
                })
            }

            fn value_type() -> $crate::tdf::ValueType {
                $crate::tdf::ValueType::Group
            }
        }
    };
}

#[cfg(test)]
mod test {
    use crate::codec::{Codec, Reader};
    use crate::types::{TdfMap, TdfOptional, VarInt, VarIntList};
    use std::fs::read;

    packet! {
        struct TestStruct {
            AB: VarInt,
            AC: String,
            AD: Vec<u8>,
            AE: MyGroup,
            AF: Vec<String>,
            AG: Vec<MyGroup>,
            AH: TdfMap<String, String>,
            AI: TdfOptional<String>,
            AK: VarIntList,
            AL: (VarInt, VarInt),
            AM: (VarInt, VarInt, VarInt)
        }
    }

    group! {
        struct MyGroup {
            ABCD: String
        }
    }

    #[test]
    fn test() {
        let mut map = TdfMap::<String, String>::new();
        map.insert("Test", "Map");
        map.insert("Other", "Test");
        map.insert("New", "Value");
        let str = TestStruct {
            AB: VarInt(12),
            AC: String::from("test"),
            AD: vec![0, 5, 12, 5],
            AE: MyGroup {
                ABCD: String::from("YES"),
            },
            AF: vec![String::from("ABC"), String::from("Abced")],
            AG: vec![
                MyGroup {
                    ABCD: String::from("YES1"),
                },
                MyGroup {
                    ABCD: String::from("YES2"),
                },
            ],
            AH: map,
            AI: TdfOptional::<String>::None,
            AK: VarIntList(vec![VarInt(1)]),
            AL: (VarInt(5), VarInt(256)),
            AM: (VarInt(255), VarInt(6000), VarInt(6743)),
        };

        let mut out = str.encode_bytes();
        let a: Option<String> = None;

        println!("{out:?}");

        let mut reader = Reader::new(&out);
        let str_out = TestStruct::decode(&mut reader).unwrap();
        println!("{str_out:?}")
    }
}

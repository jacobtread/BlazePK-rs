/// Macro for generating structures that can be encoded and decoded from bytes
/// (DONT USE THIS FOR GROUPS USE `tdf_group` because they require extra bytes)
///
/// You can only use types that implement Codec the ones implemented
/// by this library are
///
/// *Any = Any of the following types
///
/// VarInt
/// String
/// Vec<u8>
/// Group (Creates with group macro)
/// Vec<String | VarInt | Float | Group>
/// TdfMap<String | VarInt, *Any>
/// TdfOptional<*Any>
/// VarIntList
/// (VarInt, VarInt)
/// (VarInt, VarInt, VarInt)
///
/// *All field names must be in caps and no longer than 4 chars*
///
/// Example Usage
/// ```
///
/// use blaze_pk::{packet, VarInt};
///
/// packet! {
///     struct Test {
///         TEST: VarInt,
///         ALT: String,
///         BYT: Vec<u8>
///     }
/// }
///
/// ```
///
/// Generated structs can then be used as packet body's when
/// creating packets
///
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

        /// Trait fitting implementations
        impl $crate::PacketContent for $name {}
        impl $crate::Listable for $name {}

        impl $crate::Codec for $name {

            #[allow(non_snake_case)]
            fn encode(&self, output: &mut Vec<u8>) {
                $(
                    $crate::Tag::encode_from(stringify!($field), &(<$ty>::value_type()), output);
                    <$ty>::encode(&self.$field, output);
                )*
            }

            #[allow(non_snake_case)]
            fn decode(reader: &mut $crate::Reader) -> $crate::CodecResult<Self>  {
                $(
                    $crate::Tag::expect_tag(stringify!($field), &(<$ty>::value_type()), reader)?;
                    let $field = <$ty>::decode(reader)
                        .map_err(|err|$crate::CodecError::DecodeFail(stringify!($field), Box::new(err)))?;
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

        impl $crate::Listable for $name {}

        impl $crate::Codec for $name {

            #[allow(non_snake_case)]
            fn encode(&self, output: &mut Vec<u8>) {
                $(
                    $crate::Tag::encode_from(stringify!($field), &(<$ty>::value_type()), output);
                    <$ty>::encode(&self.$field, output);
                )*
                output.push(0)
            }

            #[allow(non_snake_case)]
            fn decode(reader: &mut $crate::Reader) -> $crate::CodecResult<Self> {
                $crate::Tag::take_two(reader)?;
                $(
                    $crate::Tag::expect_tag(stringify!($field), &(<$ty>::value_type()), reader)?;
                    let $field = <$ty>::decode(reader)
                        .map_err(|err|$crate::CodecError::DecodeFail(stringify!($field), Box::new(err)))?;
                )*
                $crate::Tag::discard_group(reader)?;
                Ok(Self {
                    $($field),*
                })
            }

            fn value_type() -> $crate::ValueType {
                $crate::ValueType::Group
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

        impl $crate::Listable for $name {}

        impl $crate::Codec for $name {

            #[allow(non_snake_case)]
            fn encode(&self, output: &mut Vec<u8>) {
                output.push(2);
                $(
                    $crate::Tag::encode_from(stringify!($field), &(<$ty>::value_type()), output);
                    <$ty>::encode(&self.$field, output);
                )*
                output.push(0);
            }

            #[allow(non_snake_case)]
            fn decode(reader: &mut $crate::Reader) -> $crate::CodecResult<Self> {
                $crate::Tag::take_two(reader)?;
                $(
                    $crate::Tag::expect_tag(stringify!($field), &(<$ty>::value_type()), reader)?;
                    let $field = <$ty>::decode(reader)
                        .map_err(|err|$crate::CodecError::DecodeFail(stringify!($field), Box::new(err)))?;
                )*
                $crate::Tag::discard_group(reader)?;
                Ok(Self {
                    $($field),*
                })
            }

            fn value_type() -> $crate::ValueType {
                $crate::ValueType::Group
            }
        }
    };
}

/// Macro for defining component enums for packet identification
///
/// ```
///use blaze_pk::define_components;
///define_components! {
///    Authentication (0x00) {
///        Key (0x00)
///        Alert (0x02)
///        Value (0x23)
///    }
///
///    Other (0x1) {
///        Key (0x00)
///        Alert (0x02)
///    }
/// }
/// ```
#[macro_export]
macro_rules! define_components {
    (

        $(
            $component:ident ($component_value:literal) {
                $(
                    $command:ident ($command_value:literal)
                )*
            }
        )*
    ) => {


        pub mod components {
            $(
                #[derive(Debug, Eq, PartialEq)]
                pub enum $component {
                    $($command),*,
                    Unknown(u16)
                }

                impl $crate::PacketComponent for $component {

                    fn component(&self) -> u16 {
                        $component_value
                    }

                    fn command(&self) -> u16 {
                        match self {
                            $(Self::$command => $command_value),*,
                            Self::Unknown(value) => *value,
                        }
                    }

                    fn from_value(value: u16) -> Self {
                        match value {
                            $($command_value => Self::$command),*,
                            value => Self::Unknown(value)
                        }
                    }
                }
            )*
        }
    };
}

#[cfg(test)]
mod test {
    use crate::{Codec, Reader};
    use crate::{TdfMap, TdfOptional, VarInt, VarIntList};

    packet! {
        struct TestStruct {
            AA: u8,
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
            AA: 254,
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

        let out = str.encode_bytes();

        println!("{out:?}");

        let mut reader = Reader::new(&out);
        let str_out = TestStruct::decode(&mut reader).unwrap();
        println!("{str_out:?}");

        assert_eq!(str.AB, str_out.AB);
        assert_eq!(str.AC, str_out.AC);
        assert_eq!(str.AD, str_out.AD);
        assert_eq!(str.AB, str_out.AB);
        assert_eq!(str.AB, str_out.AB);
    }
}

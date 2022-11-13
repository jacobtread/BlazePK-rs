/// Macro for generating structures that can be encoded and decoded from bytes
/// (DONT USE THIS FOR GROUPS USE `group` because they require extra bytes)
///
/// You can only use types that implement Codec the ones implemented
/// by this library are
///
/// Example Usage
/// ```
///
/// use blaze_pk::{packet, Blob};
///
/// packet! {
///     struct Test {
///         TEST test: u16,
///         ALT alt: String,
///         BYT byt: Vec<u8>
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
                $tag:ident $field:ident: $ty:ty
            ),* $(,)?
        }

    ) => {
        #[derive(Debug)]
        pub struct $name {
            $(pub $field: $ty),*
        }

        /// Trait fitting implementations
        impl $crate::codec::Codec for $name {

            fn encode(&self, output: &mut Vec<u8>) {
                $($crate::encode_field!(output, $tag, &self.$field, $ty);)*
            }

            fn decode(reader: &mut $crate::codec::Reader) -> $crate::codec::CodecResult<Self>  {
                $($crate::decode_field!(reader, $tag, $field, $ty);)*
                Ok(Self {
                    $($field),*
                })
            }
        }
    };
}

/// Macro for generating encoding for a field with with a tag and field
#[macro_export]
macro_rules! encode_field {
    ($output:ident, $tag:ident, $field:expr, $ty:ty) => {
        $crate::tag::Tag::encode_from(stringify!($tag), &(<$ty>::value_type()), $output);
        <$ty>::encode($field, $output);
    };
}

/// Macro for generating decoding for a field and tag
#[macro_export]
macro_rules! decode_field {
    ($reader:ident, $tag:ident, $field:ident, $ty:ty) => {
        let $field = $crate::tag::Tag::expect::<$ty>($reader, stringify!($tag)).map_err(|err| {
            $crate::codec::CodecError::DecodeFail(stringify!($field), Box::new(err))
        })?;
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
                $tag:ident $field:ident: $ty:ty
            ),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        #[allow(non_snake_case)]
        pub struct $name {
            $(pub $field: $ty),*
        }

        impl $crate::codec::Codec for $name {

            fn encode(&self, output: &mut Vec<u8>) {
                $($crate::encode_field!(output, $tag, &self.$field, $ty);)*
                output.push(0)
            }

            fn decode(reader: &mut $crate::codec::Reader) -> $crate::codec::CodecResult<Self> {
                $crate::tag::Tag::take_two(reader)?;
                $($crate::decode_field!(reader, $tag, $field, $ty);)*
                $crate::tag::Tag::discard_group(reader)?;
                Ok(Self {
                    $($field),*
                })
            }

            fn value_type() -> $crate::tag::ValueType {
                $crate::tag::ValueType::Group
            }
        }
    };
    (
        (2) struct $name:ident {
            $(
                $tag:ident $field:ident: $ty:ty
            ),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        #[allow(non_snake_case)]
        pub struct $name {
            $(pub $field: $ty),*
        }

        impl $crate::codec::Codec for $name {

            fn encode(&self, output: &mut Vec<u8>) {
                output.push(2);
                $($crate::encode_field!(output, $tag, &self.$field, $ty);)*
                output.push(0);
            }

            fn decode(reader: &mut $crate::Reader) -> $crate::codec::CodecResult<Self> {
                $crate::tag::Tag::take_two(reader)?;
                $($crate::decode_field!(reader, $tag, $field, $ty);)*
                $crate::tag::Tag::discard_group(reader)?;
                Ok(Self {
                    $($field),*
                })
            }

            fn value_type() -> $crate::ValueType {
                $crate::tag::ValueType::Group
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

                $(;
                    notify {

                        $(
                            $command_notify:ident ($command_notify_value:literal)
                        )*

                    }
                )?
            }
        )*
    ) => {
        #[derive(Debug, Eq, PartialEq)]
        pub enum Components {
            $($component($component),)*
            Unknown(u16, u16)
        }

        impl $crate::packet::PacketComponents for Components {


            fn values(&self)-> (u16, u16) {
                use $crate::packet::PacketComponent;
                match self {
                    $(
                        Self::$component(command) => ($component_value, command.command()),
                    )*
                    Self::Unknown(a, b) => (*a, *b),
                }
            }

            fn from_values(component: u16, command: u16, notify: bool) -> Self {
                use $crate::packet::PacketComponent;
                match component {
                    $($component_value => Self::$component($component::from_value(command, notify)),)*
                    _ => Self::Unknown(component, command),
                }
            }
        }

        $(
            #[derive(Debug, Eq, PartialEq)]
            pub enum $component {
                $($command,)*
                $($($command_notify,)*)?
                Unknown(u16)
            }

            impl $crate::packet::PacketComponent for $component {
                fn command(&self) -> u16 {
                    match self {
                        $(Self::$command => $command_value,)*
                        $(
                            $(Self::$command_notify => $command_notify_value,)*
                        )?
                        Self::Unknown(value) => *value,
                    }
                }

                fn from_value(value: u16, notify: bool) -> Self {
                    if notify {
                        match value {
                            $($($command_notify_value => Self::$command_notify,)*)?
                            value => Self::Unknown(value)
                        }
                    } else  {
                        match value {
                            $($command_value => Self::$command,)*
                            value => Self::Unknown(value)
                        }
                    }
                }
            }
        )*
    };
}

#[cfg(test)]
mod test {
    use crate::codec::{Codec, Reader};
    use crate::types::{TdfMap, TdfOptional, VarIntList};

    define_components! {
        Authentication (0x1) {

            SuperLongNameThisIs (0x2)

        }
    }

    packet! {
        struct TestStruct {
            AA aa: u8,
            AB ab: u16,
            AC ac: String,
            AD ad: Vec<u8>,
            AE ae: MyGroup,
            AF af: Vec<String>,
            AG ag: Vec<MyGroup>,
            AH ah: TdfMap<String, String>,
            AI ai: TdfOptional<String>,
            AK ak: VarIntList<u32>,
            AL al: (u8, u8),
            AM am: (u32, u32, u32)
        }
    }

    group! {
        struct MyGroup {
            ABCD abcd: String
        }
    }

    #[test]
    fn test() {
        let mut map = TdfMap::<String, String>::new();
        map.insert("Test", "Map");
        map.insert("Other", "Test");
        map.insert("New", "Value");
        let str = TestStruct {
            aa: 254,
            ab: 12,
            ac: String::from("test"),
            ad: vec![0, 5, 12, 5],
            ae: MyGroup {
                abcd: String::from("YES"),
            },
            af: vec![String::from("ABC"), String::from("Abced")],
            ag: vec![
                MyGroup {
                    abcd: String::from("YES1"),
                },
                MyGroup {
                    abcd: String::from("YES2"),
                },
            ],
            ah: map,
            ai: TdfOptional::<String>::None,
            ak: VarIntList(vec![1]),
            al: (5, 236),
            am: (255, 6000, 6743),
        };

        let out = str.encode_bytes();

        println!("{out:?}");

        let mut reader = Reader::new(&out);
        let str_out = TestStruct::decode(&mut reader).unwrap();
        println!("{str_out:?}");

        assert_eq!(str.ab, str_out.ab);
        assert_eq!(str.ac, str_out.ac);
        assert_eq!(str.ad, str_out.ad);
    }
}

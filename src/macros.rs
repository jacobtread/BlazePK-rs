/// Macro for defining component enums for packet identification
///
/// ```
///use blaze_pk::define_components;
///define_components! {
///    Authentication (0x00) {
///        Key (0x00)
///        Alert (0x02)
///        Value (0x23);
///
///        notify {
///          TestNotify (0x02)
///        }
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

        /// Hashing implementation to allow components to be used
        /// as map keys
        impl Hash for Components {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                use $crate::packet::PacketComponents;
                self.values().hash(state)
            }
        }

    };
}

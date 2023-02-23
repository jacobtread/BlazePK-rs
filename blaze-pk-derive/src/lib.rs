use darling::FromAttributes;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, punctuated::Punctuated, token::Comma, Data, DataEnum, DeriveInput, Field,
    Fields, Ident, 
};

/// Options for a component field on the components enum
#[derive(FromAttributes)]
#[darling(attributes(component), forward_attrs(allow, doc, cfg))]
struct ComponentOpts {
    /// The component target value
    target: u16,
}

/// Macro for deriving components any enum that wants to implement
/// PacketComponents must also implement Debug, Hash, PartialEq, and Eq
/// these traits are required for routing
///
/// ```
/// use blaze_pk::{PacketComponents, PacketComponent}
///
/// #[derive(Debug, Hash, PartialEq, Eq, PacketComponents)]
/// pub enum Components {
///     #[component(target = 0x1)]
///     Component1(Component1)
/// }
///
/// #[derive(Debug, Hash, PartialEq, Eq, PacketComponents)]
/// pub enum Component1 {
///     #[command(target = 0x14)]
///     Value,
///     #[command(target = 0x14, notify)]
///     NotifyValue,
/// }
///
/// ```
#[proc_macro_derive(PacketComponents, attributes(component))]
pub fn derive_componets(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    let ident: Ident = input.ident;

    // PacketComponents can only be enum types
    let data: DataEnum = match input.data {
        Data::Enum(data) => data,
        ty => panic!(
            "Expects enum for components derive dont know how to handle: {:?}",
            ty
        ),
    };

    let length = data.variants.len();
    let mut values = Vec::with_capacity(length);
    let mut from_values = Vec::with_capacity(length);

    for variant in data.variants {
        let name: Ident = variant.ident;

        // Parse the component attributes
        let target: u16 = match ComponentOpts::from_attributes(&variant.attrs) {
            Ok(value) => value.target,
            Err(err) => panic!("Unable to parse attributes for field '{}': {:?}", name, err),
        };

        // Ensure we only have one un-named field on the enum variant
        let mut fields: Punctuated<Field, Comma> = match variant.fields {
            Fields::Unnamed(fields) => fields.unnamed,
            _ => panic!("Field on '{}' must be unnamed and not unit type", name),
        };
        if fields.len() != 1 {
            panic!("Expected only 1 field on '{}' for component value", name);
        }

        // Take the enum field and its type
        let value = fields
            .pop()
            .expect("Expected one component type value")
            .into_value();

        let ty = value.ty;

        // Create the mappings for the values match
        values.push(quote! { Self::#name(value) => (#target, value.command()), });
        // Create the mappings for the from_values match
        from_values
            .push(quote! { #target => Some(Self::#name(#ty::from_value(command, notify)?)), });
    }

    // Implement the trait
    quote! {
        impl blaze_pk::packet::PacketComponents for #ident {

            fn values(&self) -> (u16, u16) {
                use blaze_pk::packet::PacketComponent;
                match self {
                    #(#values)*
                }
            }

            fn from_values(component: u16, command: u16, notify: bool) -> Option<Self> {
                use blaze_pk::packet::PacketComponent;
                match component {
                    #(#from_values)*
                    _ => None
                }
            }
        }
    }
    .into()
}

/// Options for a command field on a component
#[derive(FromAttributes)]
#[darling(attributes(command), forward_attrs(allow, doc, cfg))]
struct CommandOpts {
    /// The command target value
    target: u16,
    /// Whether the command is a notify type
    #[darling(default)]
    notify: bool,
}

/// Macro for deriving a component any enum that wants to implement
/// PacketComponent must also implement Debug, Hash, PartialEq, and Eq
/// these traits are required for routing
///
/// ```
/// use blaze_pk::{PacketComponent}
///
/// #[derive(Debug, Hash, PartialEq, Eq, PacketComponents)]
/// pub enum Component1 {
///     #[command(target = 0x14)]
///     Value,
///     #[command(target = 0x14, notify)]
///     NotifyValue,
/// }
///
/// ```
#[proc_macro_derive(PacketComponent, attributes(command))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    let ident: Ident = input.ident;

    let data: DataEnum = match input.data {
        Data::Enum(data) => data,
        ty => panic!(
            "Expects enum for component derive dont know how to handle: {:?}",
            ty
        ),
    };

    let length = data.variants.len();

    let mut from_notify_value = Vec::new();
    let mut from_normal_value = Vec::new();

    let mut command = Vec::with_capacity(length);

    for variant in data.variants {
        let name: Ident = variant.ident;
        let CommandOpts { target, notify } = match CommandOpts::from_attributes(&variant.attrs) {
            Ok(value) => value,
            Err(err) => panic!(
                "Unable to parse component options for field '{}': {:?}",
                name, err
            ),
        };

        command.push(quote! { Self::#name => #target, });

        let list = if notify {
            &mut from_notify_value
        } else {
            &mut from_normal_value
        };

        list.push(quote! { #target => Some(Self::#name), })
    }

    let from_value_notify = if from_notify_value.is_empty() {
        quote!(None)
    } else {
        quote! {
            match value {
                #(#from_notify_value)*
                _ => None
            }
        }
    };

    let from_value_normal = if from_normal_value.is_empty() {
        quote!(None)
    } else {
        quote! {
            match value {
                #(#from_normal_value)*
                _ => None
            }
        }
    };

    // Implement PacketComponent
    quote! {
        impl blaze_pk::packet::PacketComponent for #ident {
            fn command(&self) -> u16 {
                match self {
                    #(#command)*
                }
            }

            fn from_value(value: u16, notify: bool) -> Option<Self> {
                if notify {
                    #from_value_notify
                } else {
                    #from_value_normal
                }

            }
        }
    }
    .into()
}

use darling::FromAttributes;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DataEnum, DeriveInput, Ident, Type};

#[derive(FromAttributes)]
#[darling(attributes(component), forward_attrs(allow, doc, cfg))]
struct ComponentOpts {
    target: u16,
}

#[proc_macro_derive(Components, attributes(component))]
pub fn derive_componets(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    let ident: Ident = input.ident;

    let en: DataEnum = match input.data {
        syn::Data::Enum(en) => en,
        ty => panic!(
            "Expects enum for components derive dont know how to handle: {:?}",
            ty
        ),
    };

    let mut components = Vec::new();

    for variant in en.variants {
        let name = variant.ident;
        let opts = match ComponentOpts::from_attributes(&variant.attrs) {
            Ok(value) => value,
            Err(err) => panic!(
                "Unable to parse component options for field '{}': {:?}",
                name, err
            ),
        };

        let ty: Type = match variant.fields {
            syn::Fields::Unnamed(unnamed) => {
                let mut values = unnamed.unnamed;
                if values.len() > 1 {
                    panic!("Only expected one component type for {}", name);
                }
                let value = values
                    .pop()
                    .expect("Expected one component type")
                    .into_value();

                value.ty
            }
            syn::Fields::Named(_) => panic!("Not expecting named variants for {}", name),
            syn::Fields::Unit => panic!("{} missing component type", name),
        };
        components.push((name, opts.target, ty))
    }

    let values: Vec<_> = components
        .iter()
        .map(|(name, target, _)| {
            quote! {
                Self::#name(value) => (#target, value.command()),
            }
        })
        .collect();

    let from_values = components.iter().map(|(name, target, ty)| {
        quote! {
            #target => Some(Self::#name(#ty::from_value(command, notify)?)),
        }
    });

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

#[derive(FromAttributes)]
#[darling(attributes(command), forward_attrs(allow, doc, cfg))]
struct CommandOpts {
    target: u16,
    #[darling(default)]
    notify: bool,
}

#[proc_macro_derive(Component, attributes(command))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    let ident: Ident = input.ident;

    let en: DataEnum = match input.data {
        syn::Data::Enum(en) => en,
        ty => panic!(
            "Expects enum for component derive dont know how to handle: {:?}",
            ty
        ),
    };

    let mut s_mappings = Vec::new();
    let mut n_mappings = Vec::new();

    for variant in en.variants {
        let name = variant.ident;
        let opts = match CommandOpts::from_attributes(&variant.attrs) {
            Ok(value) => value,
            Err(err) => panic!(
                "Unable to parse component options for field '{}': {:?}",
                name, err
            ),
        };

        let target = if opts.notify {
            &mut n_mappings
        } else {
            &mut s_mappings
        };
        target.push((name, opts.target))
    }

    let mut command_match = Vec::new();

    for (name, target) in s_mappings.iter().chain(n_mappings.iter()) {
        command_match.push(quote! { Self::#name => #target, });
    }

    let command = s_mappings
        .iter()
        .chain(n_mappings.iter())
        .map(|(name, target)| quote! { Self::#name => #target, });

    let n_conv: Vec<_> = n_mappings
        .iter()
        .map(|(name, target)| quote! { #target => Some(Self::#name), })
        .collect();

    let s_conv: Vec<_> = s_mappings
        .iter()
        .map(|(name, target)| quote! { #target => Some(Self::#name), })
        .collect();

    let from_value = match (s_conv.is_empty(), n_conv.is_empty()) {
        (true, true) => quote! { None },
        (true, false) => quote! {
            if !notify {
                return None;
            }

            match value {
                #(#n_conv)*
                _ => None
            }
        },
        (false, true) => quote! {
            if notify {
                return None;
            }

            match value {
                #(#s_conv)*
                _ => None
            }
        },
        (false, false) => quote! {
            if notify {
                match value {
                    #(#n_conv)*
                    _ => None
                }
            } else {
                match value {
                    #(#s_conv)*
                    _ => None
                }
            }
        },
    };

    quote! {
        impl blaze_pk::packet::PacketComponent for #ident {

            fn command(&self) -> u16 {
                match self {
                    #(#command)*
                }
            }

            fn from_value(value: u16, notify: bool) -> Option<Self> {
                #from_value
            }

        }
    }
    .into()
}

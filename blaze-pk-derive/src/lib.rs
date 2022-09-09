use proc_macro::TokenStream;
use std::process::id;
use quote::{quote, ToTokens};
use syn;
use syn::{Attribute, Data, DeriveInput, GenericArgument, LitStr, Meta, MetaNameValue, parse_macro_input, PathArguments, Type};

#[proc_macro_derive(TdfStruct, attributes(tag))]
pub fn tdf_struct_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    impl_tdf_struct_derive(input)
}

struct TagAttr {
    value: String,
}

fn get_attr_name_meta(attr: &Attribute) -> Option<MetaNameValue> {
    if let Ok(attr_meta) = attr.parse_meta() {
        if let Meta::NameValue(name_value) = attr_meta {
            return Some(name_value);
        }
    }
    None
}


fn get_tag_attribute(
    field_name: &String,
    attributes: &Vec<Attribute>,
) -> String {
    let value = attributes
        .iter()
        .find(|attr| attr.path.is_ident("tag"))
        .expect(&format!("Field '{}' is missing tag attribute", field_name));

    let tag_meta = value.parse_meta()
        .expect(&format!("Unable to parse tag on field '{}'", field_name));

    let value = value.parse_args::<LitStr>()
        .expect(&format!("Unable to parse tag name on field '{}'", field_name))
        .value();

    return value;
}

fn get_type_name(
    field_name: &String,
    field_type: &Type,
) -> String {
    if let Type::Path(type_path) = field_type {
        if let Some(ident) = type_path.path.get_ident() {
            return ident.to_string();
        } else {
            let path_parts = type_path.path.segments.last()
                .expect(&format!("Don't know how to parse path for {}", field_name));

            let mut name = path_parts.ident.to_string();

            if let PathArguments::AngleBracketed(vargs) = &path_parts.arguments {
                let first_arg = vargs.args.first()
                    .expect(&format!("Generic type for field '{}' missing value", field_name));

                if let GenericArgument::Type(generic_type) = first_arg {
                    let value = get_type_name(field_name, generic_type);
                    name.push('<');
                    name.push_str(&value);
                    name.push('>');
                }

                return name;
            } else {
                panic!("Don't know how to handle type for field '{}'", field_name)
            }
        }
    } else {
        panic!("Don't know how to handle type ")
    }
}

fn impl_tdf_struct_derive(input: DeriveInput) -> TokenStream {
    let data = input.data;
    let name = input.ident;

    if let Data::Struct(stru) = data {
        let mut serial_body: Vec<TokenStream> = Vec::new();
        let mut deserial_body: Vec<TokenStream> = Vec::new();

        for field in stru.fields {
            let field_name = field.ident
                .expect("Expected field to ")
                .to_string();
            let tag_name = get_tag_attribute(&field_name, &field.attrs);
            let type_name = get_type_name(&field_name, &field.ty);

            println!("field '{}' tagged '{}' is of type '{}'", field_name, tag_name, type_name);
        }

        let expanded = quote!(
            impl TdfStruct for #name {
                fn serialize(&self) -> TdfResult<Vec<Tdf>> {
                    return Ok(Vec::new())
                }

                fn deserialize(contents: Vec<Tdf>) -> Self {
                    return Self {
                        name: "".to_string(),
                        v: 0,
                        a: 0,
                        b: false,
                    }
                }
            }
        );
        println!("PEAWWd");
        TokenStream::from(expanded)
    } else {
        panic!("Not struct")
    }
}

// #[proc_macro_derive(HelloMacro)]
// pub fn hello_macro_derive(input: TokenStream) -> TokenStream {
//     // Construct a representation of Rust code as a syntax tree
//     // that we can manipulate
//     let ast = syn::parse(input).unwrap();
//
//     // Build the trait implementation
//
//     impl_hello_macro(&ast)
// }
extern crate prost_build;
//extern crate cbindgen;

use std::{io};
use std::path::Path;
use std::fs;

use std::io::prelude::*;

extern crate serde;
extern crate serde_json;
use serde::{Deserialize};

use std::fs::File;
use std::io::Read;
// BTreeMaps preserve the order of keys. HashMaps don't preserve the order of keys.
// Since components.proto is rebuilt every time validator-rust is compiled,
//     the proto field ids are shuffled if options are stored in a HashMap
// Options are stored in a BTreeMap to prevent desynchronization of the proto ids
//     between the validator build, and the validator build as a dependency of the runtime
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::iter::FromIterator;


extern crate build_deps;


#[derive(Deserialize, Debug)]
struct ComponentJSON {
    id: String,
    name: String,
    arguments: BTreeMap<String, ArgumentJSON>,
    options: BTreeMap<String, ArgumentJSON>,
    #[serde(rename(serialize = "return", deserialize = "return"))]
    arg_return: ArgumentJSON,
    description: Option<String>
}

#[derive(Deserialize, Debug)]
struct ArgumentJSON {
    nature: Option<Vec<String>>,
    #[serde(rename(serialize = "type", deserialize = "type"))]
    arg_type: Option<String>,
    default: Option<String>,
    description: Option<String>
}

fn stringify_argument((name, argument): (&String, &ArgumentJSON)) -> String {
    let mut response = format!("* `{}` - {}", name, argument.arg_type.as_ref().unwrap_or(&"".to_string()));
    if let Some(description) = &argument.clone().description {
        response.push_str(&format!(" - {}", description));
    }
    response
}

fn doc(text: &Option<String>, prefix: &str) -> String {
    match text {
        Some(text) => text.lines().map(|line| format!("{}// {}", prefix, line))
            .collect::<Vec<String>>().join("\n"),
        None => "".to_string()
    }
}

fn main() {
    // Enumerate component json files as relevant resources to the compiler
    build_deps::rerun_if_changed_paths( "../prototypes/components/*" ).unwrap();
    // Adding the parent directory "data" to the watch-list will capture new-files being added
    build_deps::rerun_if_changed_paths( "../prototypes/components" ).unwrap();
    build_deps::rerun_if_changed_paths( "../prototypes/base.proto" ).unwrap();
    build_deps::rerun_if_changed_paths( "../prototypes/api.proto" ).unwrap();
    build_deps::rerun_if_changed_paths( "../prototypes/value.proto" ).unwrap();

    let components_dir = "../prototypes/components/";
    let components_proto_path = "../prototypes/components.proto";
    let components_doc_path = "src/docs/components.rs";

    let paths = fs::read_dir(&Path::new(components_dir))
        .expect("components directory was not found");

    let components = paths
        // ignore invalid dirs
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension() == Some(OsStr::new("json")))
        .map(|entry| {
            let mut file: File = File::open(entry.path())?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;

            // Deserialize and print Rust data structure.
            let data: ComponentJSON = serde_json::from_str(&contents)?;
            Ok(data)
        })
        .collect::<Result<Vec<ComponentJSON>, io::Error>>().unwrap();

    let proto_text_header = r#"
// This file is automatically generated. Do not edit. Edit the component JSON's instead.

syntax = "proto3";

package whitenoise;
import "value.proto";

message Component {
    // uint32 value is source_node_id
    map<string, uint32> arguments = 1;
    // if true, then don't include the evaluation for this component in the release
    bool omit = 2;
    // for interactive analyses
    uint32 batch = 3;

    oneof variant {
    "#.to_string();

    let proto_text_variants = components.iter().enumerate()
        .map(|(id, component)| format!("        {} {} = {};", component.id, component.name, id + 100))
        .collect::<Vec<String>>().join("\n");

    let proto_text_messages = components.iter()
        .map(|component| {

            // code gen for options
            let text_options = component.options.iter().enumerate().map(|(id, (name, spec))| {
                format!("{}\n    {} {} = {};",
                        doc(&spec.description, "    "),
                        spec.arg_type.clone().unwrap(),
                        name,
                        id + 1)
            }).collect::<Vec<String>>().join("\n");

            let mut component_description = format!("{} Component", component.id);
            if let Some(description) = component.description.clone() {
                component_description.push_str(&format!("\n\n{}", description));
            }

            component_description.push_str(&format!("\n\nThis struct represents an abstract computation. Arguments are provided via the graph. Additional options are set via the fields on this struct. The return is the result of the {} on the arguments.", component.name));

            let component_arguments = match component.arguments.is_empty() {
                true => "".to_string(),
                false => format!("\n\n# Arguments\n{}", component.arguments.iter()
                    .map(stringify_argument)
                    .collect::<Vec<String>>().join("\n"))
            };
            // options are already listed once under the struct fields
//            let component_options = match component.options.is_empty() {
//                true => "".to_string(),
//                false => format!("\n\n# Options\n{}", component.options.iter()
//                    .map(stringify_argument)
//                    .collect::<Vec<String>>().join("\n"))
//            };
            let component_returns = format!("\n\n# Returns\n{}", stringify_argument((&"Value".to_string(), &component.arg_return)));

            let text_component_header = doc(&Some(vec![component_description, component_arguments, component_returns].join("")), "");

            format!("{}\nmessage {} {{\n{}\n}}",
                    // code gen for the header
                    text_component_header,
                    component.id,
                    text_options)
        })
        .collect::<Vec<String>>().join("\n\n");

    let proto_text = format!("{}\n{}\n    }}\n}}\n\n{}", proto_text_header, proto_text_variants, proto_text_messages);
//    println!("{}", proto_text);

    // overwrite/remove the components.proto file
    {
        fs::remove_file(components_proto_path).ok();
        let mut file = File::create(components_proto_path).unwrap();
        file.write(proto_text.as_bytes())
            .expect("Unable to write components.proto file.");
        file.flush().unwrap();
    }
//    panic to prevent stdout from being masked
//    panic!("You can't suppress me rustc!");

    let mut config = prost_build::Config::new();
    config.type_attribute("whitenoise.Component.variant", "#[derive(derive_more::From)]");
    config.compile_protos(
        &[
            "../prototypes/api.proto",
            "../prototypes/base.proto",
            "../prototypes/components.proto",
            "../prototypes/value.proto"
        ],
        &["../prototypes/"]).unwrap();


    let component_docs_text_header = r#"
// [//]: # (This file is automatically generated. Do not edit. Edit the component JSON's instead.)
//! All of the components available in the library are listed below.
//! The components may be strung together in arbitrary directed graphs (called analyses), and only verifiably DP analyses and data are released.
//!
//! | Component ID | Bindings Name | Inputs |
//! |--------------|---------------|--------|  "#.to_string();
    let component_docs_text_table = components.iter()
        .map(|component| {

            let mut inputs = BTreeSet::from_iter(&mut component.arguments.keys());
            inputs.append(&mut BTreeSet::from_iter(&mut component.options.keys()));
            let inputs = inputs.iter()
                .map(|v| format!("`{}`", v))
                .collect::<Vec<String>>().join(", ");

            format!("//! | [{}](../../proto/struct.{}.html) | {} | {} |  ",
                    component.id, component.id, component.name, inputs)
        })
        .collect::<Vec<String>>().join("\n");

    let component_docs = format!("{}\n{}", component_docs_text_header, component_docs_text_table);

    {
        fs::create_dir_all("src/docs/").ok();
        fs::remove_file(components_doc_path).ok();
        let mut file = File::create(components_doc_path).unwrap();
        file.write(component_docs.as_bytes())
            .expect("Unable to write components.rs doc file.");
        file.flush().unwrap();
    }


//    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
//
//    cbindgen::generate_with_config(
//        crate_dir,
//        cbindgen::Config::from_file("cbindgen.toml").unwrap())
//        .expect("Unable to generate bindings")
//        .write_to_file("api.h");
}
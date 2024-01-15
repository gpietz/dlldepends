use std::{env, fs, str};
use std::fmt::{Debug, Display, Formatter};
use std::path::Path;
use quick_xml::{Reader, events::Event};

#[derive(Debug, PartialEq)]
enum ReferenceType {
    None,
    Reference,
    PackageReference,
    ProjectReference,
}

impl Display for ReferenceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            ReferenceType::None => "None",
            ReferenceType::Reference => "Reference",
            ReferenceType::PackageReference => "PackageReference",
            ReferenceType::ProjectReference => "ProjectReference",
        };
        write!(f, "{}", text)
    }
}

struct ReferenceInfo {
    project_path: String,
    reference_type: ReferenceType,
}

const DEFAULT_SLN_PATH: &str = r#"d:\Development\Projects\StreamInfoHub\StreamInfoHub.sln"#;
const DEFAULT_DLL_NAME: &str = r#"Grpc.Tools"#;


fn main() {
    let mut args: Vec<String> = env::args().collect();

    // Add default parameters for testing purposes
    args.push(DEFAULT_SLN_PATH.to_string());
    args.push(DEFAULT_DLL_NAME.to_string());

    if args.len() != 3 {
        println!("Usage: dlldepends <solution file path> <dll name>")
    }

    let solution_path = &args[1];
    let dll_name = &args[2];

    let found_projects = get_project_paths(&solution_path)
        .into_iter()
        .filter_map(|project_path| {
            let ref_type = check_dependency(&project_path, dll_name.clone());
            match ref_type {
                ReferenceType::None => None,
                _ => Some(ReferenceInfo {
                    project_path,
                    reference_type: ref_type
                }),
            }
        })
        .collect::<Vec<_>>();

    if found_projects.is_empty() {
        println!("No dependencies to \"{dll_name}\" were found in the project folder.");
    } else {
        println!("{} references to \"{dll_name}\" were found in the project folder.", found_projects.len());
    }
}

fn get_project_paths(solution_path: &str) -> Vec<String> {
    let solution_dir = Path::new(solution_path).parent().unwrap().to_path_buf();
    let lines = fs::read_to_string(solution_path)
        .expect("Unable to read solution file")
        .lines()
        .map(String::from)
        .collect::<Vec<String>>();

    lines.iter()
        .filter(|line| line.starts_with("Project("))
        .map(|line| {
            let parts: Vec<&str> = line.split(',').collect();
            let relative_path = parts[1].trim().trim_matches('"');
            solution_dir.join(relative_path).to_string_lossy().into_owned()
        })
        .collect()
}

fn check_dependency(project_path: &str, mut dll_name: String) -> ReferenceType {
    println!("Reading project: {}", project_path);

    let xml = fs::read_to_string(project_path).expect("Unable to read project file");
    let mut reader = Reader::from_str(&xml);
    reader.trim_text(true);

    if dll_name.to_lowercase().ends_with(".dll") {
        dll_name = Path::new(&dll_name)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
    }

    // get the namespace from xml
    let mut buf = Vec::new();
    let mut ns = String::new();
    let mut sdk = String::new();

    'outer: loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                for attribute in e.attributes().filter_map(Result::ok) {
                    let key_name = str::from_utf8(attribute.key.as_ref()).unwrap().to_lowercase();
                    match key_name.as_ref() {
                        "xmlns" => {
                            ns = str::from_utf8(attribute.value.as_ref()).unwrap().to_string();
                            buf.clear();
                            break 'outer;
                        },
                        "sdk" => {
                            sdk = str::from_utf8(attribute.value.as_ref()).unwrap().to_string();
                            buf.clear();
                            break 'outer;
                        },
                        _ => ()
                    };
                }
            },
            Ok(Event::Eof) => break 'outer,
            _ => (),
        }
    }

    let reference_types = vec![
        ReferenceType::Reference,
        ReferenceType::PackageReference,
        ReferenceType::ProjectReference
    ];
    for rt in reference_types {
        if has_reference(&xml, &rt, &dll_name) {
            return rt;
        }
    }

    ReferenceType::None
}

fn has_reference(xml: &str, reference_type: &ReferenceType, dll_name: &str) -> bool {
    let expected_element_name = reference_type.to_string();
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    println!("## scanning: {dll_name} ({reference_type})");
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref element)) | Ok(Event::Empty(ref element)) => {
                let element_name = element.name();
                let element_name = str::from_utf8(element_name.as_ref()).unwrap();
                if element_name == expected_element_name {
                    for attribute in element.attributes().filter_map(Result::ok) {
                        if let Ok(key) = str::from_utf8(attribute.key.as_ref()) {
                            if key.to_lowercase() == "include" {
                                let include_value = str::from_utf8(attribute.value.as_ref()).unwrap();
                                println!("-----> {} vs {}", include_value, dll_name);
                                if include_value == dll_name {
                                    buf.clear();
                                    return true;
                                }
                                if *reference_type == ReferenceType::ProjectReference {
                                    println!("--> ProjectRef: {}", include_value)
                                }
                            }
                        }
                    }
                }
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("Error reading XML: {}", e);
                break;
            }
            _ => ()
        }
    }
    false
}

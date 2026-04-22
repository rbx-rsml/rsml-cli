use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use rbx_rsml::{
    RsmlCompiler, RsmlParser, compiler::tree_node::CompiledRsml, lexer::Token, parser::Construct,
};
use rbx_types::{Attributes, Variant};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use serde_json::{Serializer as JsonSerializer, json, ser::PrettyFormatter};

use crate::{NormalizePath, WatcherContext, guarded_unwrap, luaurc::Luaurc};

#[derive(Deserialize)]
pub struct StyleSheet {
    id: String,
    attributes: Attributes,
    children: Vec<Child>,
}

impl Serialize for StyleSheet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut x = serializer.serialize_struct("StyleSheet", 4)?;
        x.serialize_field("className", "StyleSheet")?;
        x.serialize_field("id", &self.id)?;
        x.serialize_field("attributes", &self.attributes)?;
        x.serialize_field("children", &self.children)?;
        x.end()
    }
}

#[derive(Deserialize)]
struct StyleRule {
    name: Option<String>,
    attributes: Attributes,
    properties: BTreeMap<String, Variant>,
    children: Vec<Child>,
}

impl Serialize for StyleRule {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut x = if let Some(name) = &self.name {
            let mut x = serializer.serialize_struct("StyleRule", 5)?;
            x.serialize_field("name", &name)?;
            x
        } else {
            serializer.serialize_struct("StyleRule", 4)?
        };

        x.serialize_field("className", "StyleRule")?;
        x.serialize_field("attributes", &self.attributes)?;
        x.serialize_field("properties", &self.properties)?;
        x.serialize_field("children", &self.children)?;

        x.end()
    }
}

#[derive(Deserialize)]
struct StyleDerive {
    name: String,
    stylesheet: String,
}

impl Serialize for StyleDerive {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut x = serializer.serialize_struct("StyleDerive", 3)?;
        x.serialize_field("className", "StyleDerive")?;
        x.serialize_field("name", &self.name)?;
        x.serialize_field(
            "attributes",
            &json!({
                "Rojo_Target_StyleSheet": &self.stylesheet
            }),
        )?;
        x.end()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Child {
    StyleRule(StyleRule),
    StyleDerive(StyleDerive),
}

fn extract_derive_paths(source: &str) -> Vec<String> {
    let parsed = RsmlParser::from_source(source);
    parsed
        .ast
        .iter()
        .filter_map(|c| {
            if let Construct::Derive {
                body: Some(body), ..
            } = c
            {
                if let Construct::Node { node } = body.as_ref() {
                    return match node.token.value() {
                        Token::StringSingle(s) => Some(s.to_string()),
                        Token::StringMulti(ms) => Some(ms.content.to_string()),
                        _ => None,
                    };
                }
            }
            None
        })
        .collect()
}

fn resolve_derive_alias(
    derived_path: &str,
    current_path: &Path,
    luaurc: Option<&mut (PathBuf, Luaurc)>,
) -> PathBuf {
    let path = 'core: {
        let path = PathBuf::from(derived_path).normalize();
        let (_, luaurc) = guarded_unwrap!(luaurc, break 'core path);

        let mut components = path.components();

        let component = guarded_unwrap!(components.next(), break 'core path);
        let component_str = component.as_os_str().to_string_lossy();

        if component_str.starts_with("@")
            && let Some(alias) = luaurc.aliases.get(&component_str.as_ref()[1..])
        {
            let mut path = PathBuf::from(alias);

            path.push(components);

            return path;
        } else {
            path
        }
    };

    current_path.join("../").join(path)
}

fn resolve_derive(
    content: &str,
    current_path: &Path,
    luaurc: Option<&mut (PathBuf, Luaurc)>,
) -> Option<PathBuf> {
    let content = content.trim();
    let mut path = resolve_derive_alias(content, current_path, luaurc);
    path.set_extension("rsml");

    match path.canonicalize() {
        Ok(canonicalized) => {
            if &canonicalized == current_path {
                None
            } else {
                Some(canonicalized)
            }
        }

        Err(_) => None,
    }
}

fn convert_children(compiled: &mut CompiledRsml, children: Vec<usize>) -> Vec<Child> {
    children
        .iter()
        .map(|child_idx| {
            let child = compiled.take_node(*child_idx).unwrap();
            let selector = child.selector;

            Child::StyleRule(StyleRule {
                name: selector.clone(),

                attributes: child.attributes,

                properties: {
                    let mut properties = BTreeMap::new();

                    if let Some(selector) = selector {
                        properties.insert("Selector".to_string(), Variant::String(selector));
                    };

                    if let Some(priority) = child.priority {
                        properties.insert("Priority".to_string(), Variant::Int32(priority));
                    };

                    properties.insert(
                        "PropertiesSerialize".to_string(),
                        Variant::Attributes(child.properties),
                    );

                    properties
                },

                children: convert_children(compiled, child.child_rules),
            })
        })
        .collect::<Vec<Child>>()
}

fn track_derive_dependencies(
    derive_path: PathBuf,
    path: &Path,
    already_tracked: &mut HashSet<PathBuf>,
    watcher: &mut WatcherContext,
) {
    if let Ok(derive_content) = fs::read_to_string(&derive_path) {
        let derives = extract_derive_paths(&derive_content);
        for derive in derives {
            let derive_path = guarded_unwrap!(
                resolve_derive(&derive, path, watcher.luaurc.as_mut()),
                continue
            );

            if already_tracked.contains(&derive_path) {
                continue;
            }

            track_derive_dependencies(derive_path.clone(), path, already_tracked, watcher);

            already_tracked.insert(derive_path);
        }
    }

    watcher.dependencies.insert(path.to_path_buf(), derive_path);
}

pub fn rsml_to_model_json(path: &Path, watcher: &mut WatcherContext) -> String {
    let content = fs::read_to_string(path).unwrap();

    let derive_strings = extract_derive_paths(&content);

    let mut already_tracked: HashSet<PathBuf> = HashSet::new();

    let derives_children = derive_strings
        .iter()
        .filter_map(|derive| {
            let derive_path = guarded_unwrap!(
                resolve_derive(&derive, path, watcher.luaurc.as_mut()),
                return None
            );

            track_derive_dependencies(derive_path.clone(), path, &mut already_tracked, watcher);

            Some(Child::StyleDerive(StyleDerive {
                name: derive_path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
                stylesheet: derive_path
                    .strip_prefix(&watcher.input_dir)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            }))
        })
        .collect::<Vec<Child>>();

    let mut compiled = RsmlCompiler::from_source(&content);

    let rsml_root = compiled.take_root().unwrap();

    let mut children = convert_children(&mut compiled, rsml_root.child_rules);
    children.extend(derives_children);

    let style_sheet = StyleSheet {
        id: path
            .normalize()
            .strip_prefix(&watcher.input_dir)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string(),
        attributes: rsml_root.attributes,
        children,
    };

    let formatter = PrettyFormatter::with_indent(b"    ");
    let mut buffer = Vec::new();
    let mut serializer = JsonSerializer::with_formatter(&mut buffer, formatter);
    style_sheet.serialize(&mut serializer).unwrap();
    String::from_utf8(buffer).unwrap()
}

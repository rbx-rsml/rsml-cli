use std::{collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}};

use normalize_path::NormalizePath;
use rbx_types::{Attributes, Variant};
use rbx_rsml::{lex_rsml, lex_rsml_derives, lex_rsml_macros, parse_rsml, parse_rsml_derives, parse_rsml_macros, MacroGroup, TreeNodeGroup, BUILTIN_MACROS};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use serde_json::{json, ser::PrettyFormatter, Serializer as JsonSerializer};

use crate::WatcherContext;


#[derive(Deserialize)]
pub struct StyleSheet {
    id: String,
    attributes: Attributes,
    children: Vec<Child>,
}

impl Serialize for StyleSheet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer
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
    properties: HashMap<String, Variant>,
    children: Vec<Child>
}

impl Serialize for StyleRule {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer
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
    stylesheet: String
}

impl Serialize for StyleDerive {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        let mut x = serializer.serialize_struct("StyleDerive", 4)?;
        x.serialize_field("className", "StyleDerive")?;
        x.serialize_field("name", &self.name)?;
        x.serialize_field("attributes", &json!({
            "Rojo_Target_StyleSheet": &self.stylesheet
        }))?;
        x.end()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Child {
    StyleRule(StyleRule),
    StyleDerive(StyleDerive)
}

fn convert_children(parsed_rsml: &mut TreeNodeGroup, children: Vec<usize>) -> Vec<Child> {
    children
        .iter().map(|child_idx| {
            let child: rbx_rsml::TreeNode = parsed_rsml.take_node(*child_idx).unwrap();
            let selector = child.selector;

            Child::StyleRule(StyleRule {
                name: child.name.or(selector.clone()),

                attributes: child.attributes,

                properties: {
                    let mut properties = HashMap::new();

                    if let Some(selector) = selector {
                        properties.insert("Selector".to_string(), Variant::String(selector));
                    };

                    if let Some(priority) = child.priority {
                        properties.insert("Priority".to_string(), Variant::Int32(priority));
                    };

                    properties.insert("PropertiesSerialize".to_string(), Variant::Attributes(child.properties));

                    properties
                },

                children: convert_children(parsed_rsml, child.child_rules)
            })
        })
        .collect::<Vec<Child>>()
}

fn derive_to_path_buf(derive: &str, parent_path: &Path) -> PathBuf {
    let derive = if !derive.ends_with(".rsml") { &format!("{}.rsml", derive) } else { derive };
    parent_path.join(Path::new(derive)).normalize()
}

fn parse_macros_from_derives(
    derive_path: PathBuf, path: &Path, parent_path: &Path, already_parsed_derives: &mut HashSet<PathBuf>,
    macro_group: &mut MacroGroup, watcher: &mut WatcherContext
) {
    // If the file is valid then we add its macros to the macro group,
    // then we attempt to add all of the macros from the files dependencies
    // to the macro group.
    if let Ok(derive_content) = fs::read_to_string(&derive_path) {
        parse_rsml_macros(macro_group, &mut lex_rsml_macros(&derive_content));

        let derives = parse_rsml_derives(&mut lex_rsml_derives(&derive_content));
        for derive in derives {
            let derive_path = derive_to_path_buf(&derive, path);

            if already_parsed_derives.contains(&derive_path) { continue }

            parse_macros_from_derives(
                derive_path.clone(), path, parent_path, already_parsed_derives,
                macro_group, watcher
            );

            already_parsed_derives.insert(derive_path);
        }
    };

    watcher.dependencies.insert(derive_path, path.to_path_buf());
}

pub fn rsml_to_model_json(path: &Path, watcher: &mut WatcherContext) -> String {
    let parent_path = path.parent().unwrap();
    let content = fs::read_to_string(path).unwrap();

    let mut macro_group = BUILTIN_MACROS.clone();

    let derives = parse_rsml_derives(&mut lex_rsml_derives(&content));

    let mut already_parsed_derives: HashSet<PathBuf> = HashSet::new();

    let derives_children = derives.iter()
        .map(|derive| {
            let derive_path = derive_to_path_buf(&derive, path);

            parse_macros_from_derives(
                derive_path.clone(), path, parent_path, &mut already_parsed_derives,
                &mut macro_group, watcher
            );

            Child::StyleDerive(StyleDerive {
                name: derive_path.file_stem().unwrap().to_str().unwrap().to_string(),
                stylesheet: derive_path.strip_prefix(&watcher.input_dir).unwrap().to_str().unwrap().to_string()
            })
        })
        .collect::<Vec<Child>>();

    parse_rsml_macros(&mut macro_group, &mut lex_rsml_macros(&content));
    let mut parsed_rsml = parse_rsml(&mut lex_rsml(&content), &macro_group);

    let rsml_root = parsed_rsml.take_root().unwrap();

    let mut children = convert_children(&mut parsed_rsml, rsml_root.child_rules); 
    children.extend(derives_children);

    let style_sheet = StyleSheet {
        id: path.normalize().strip_prefix(&watcher.input_dir).unwrap().to_str().unwrap().to_string(),
        attributes: rsml_root.attributes,
        children: children,
    };

    let formatter = PrettyFormatter::with_indent(b"    ");
    let mut buffer = Vec::new();
    let mut serializer = JsonSerializer::with_formatter(&mut buffer, formatter);
    style_sheet.serialize(&mut serializer).unwrap();
    let json_string = String::from_utf8(buffer).unwrap();

    json_string
}
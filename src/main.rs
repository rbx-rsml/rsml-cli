use notify::{event::{CreateKind, ModifyKind}, Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rbx_rsml::{lex_rsml, parse_rsml, Arena, TokenTreeNode};
use rbx_types::{Color3, Variant};
use regex::Regex;
use std::{collections::HashMap, ffi::OsStr, fs::{read_to_string, write}, path::Path, sync::LazyLock};
use sha2::{Sha256, Digest};
use normalize_path::NormalizePath;


const ENUM_REGEX: LazyLock<Regex> = LazyLock::new(|| { Regex::new(r"Enum\.[^ \.]+\.[^ \.]+").unwrap() });

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let path = std::env::args()
        .nth(1)
        .expect("Argument 1 needs to be a path");

    log::info!("Watching {path}");

    if let Err(error) = watch(path) {
        log::error!("Error: {error:?}");
    }
}

fn stringify_path(path: &Path) -> String {
    let stringified = path.normalize().to_str().unwrap()
        // On windows the path string uses `\` as a separator instead of `/`.
        .replace(r"\", r"/");

    stringified
}

fn string_to_ref(str: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(str);
    let hash = hasher.finalize();

    format!("{:032x}", u128::from_be_bytes(hash[..16].try_into().unwrap()))
}

fn stringify_color3(col3: &Color3) -> String {
    format!("Color3.new({}, {}, {})", col3.r, col3.g, col3.b)
}

fn stringify_variant(variant: &Variant) -> String {
    match variant {
        Variant::BrickColor(brick_color) => format!("BrickColor.new({})", brick_color),
        Variant::Color3(col3) => stringify_color3(col3),
        Variant::ColorSequence(color_seq) => format!(
            "ColorSequence.new {{ \n{}\n}}",
            color_seq.keypoints.iter()
                .map(|keypoint| format!("   ColorSequenceKeypoint.new({}, {}),", keypoint.time, stringify_color3(&keypoint.color)))
                .collect::<Vec<String>>()
                .join("\n")
        ),
        Variant::Enum(rbx_enum) => format!("{}", rbx_enum.to_u32()),
        Variant::Font(font) => format!("Font.fromName(\"{}\", \"{:#?}\", \"{:#?}\")", font.family, font.weight, font.style),
        Variant::NumberRange(num_range) => format!("NumberRange.new({}, {})", num_range.min, num_range.max),
        Variant::NumberSequence(num_seq) => format!(
            "NumberSequence.new {{ \n{}\n}}",
            num_seq.keypoints.iter()
                .map(|keypoint| format!("   ColorSequenceKeypoint.new({}, {}),", keypoint.time, keypoint.value))
                .collect::<Vec<String>>()
                .join("\n")
        ),
        Variant::Rect(rect) => {
            let (min, max) = (rect.min, rect.max);
            format!("Rect.new({}, {}, {}, {})", min.x, min.y, max.x, max.y)
        },
        Variant::Region3(reg3) => {
            let (min, max) = (reg3.min, reg3.max);
            format!("Region3.new({}, {}, {}, {})", min.x, min.y, max.x, max.y)
        },
        Variant::Region3int16(reg3int16) => {
            let (min, max) = (reg3int16.min, reg3int16.max);
            format!("Region3int16.new({}, {}, {}, {})", min.x, min.y, max.x, max.y)
        },
        Variant::UDim(udim) => format!("UDim.new({}, {})", udim.scale, udim.offset),
        Variant::UDim2(udim2) => {
            let (x, y) = (udim2.x, udim2.y);
            format!("UDim2.new({}, {}, {}, {})", x.scale, x.offset, y.scale, y.offset)
        },
        Variant::Vector2(vec2) => format!("Vector2.new({}, {})", vec2.x, vec2.y),
        Variant::Vector2int16(vec2int16) => format!("Vector2int16.new({}, {})", vec2int16.x, vec2int16.y),
        Variant::Vector3(vec3) => format!("Vector3.new({}, {}, {})", vec3.x, vec3.y, vec3.z),
        Variant::Vector3int16(vec3int16) => format!("Vector3int16.new({}, {}, {})", vec3int16.x, vec3int16.y, vec3int16.z),
        Variant::String(str) => {
            if ENUM_REGEX.is_match(str) { str.to_owned() }
            else { format!("\"{}\"", str) }
        },
        Variant::Float32(f32) => f32.to_string(),
        Variant::Bool(bool) => if *bool { "true".to_string() } else { "false".to_string() },
        _ => "nil -- Failed to stringify this property.".to_string()
    }
}

fn stringify_properties(properties: &HashMap<&str, Variant>) -> String {
    let mut stringified = String::from("{");

    for (name, value) in properties {
        stringified += &format!("\n    [\"{}\"] = {},", name, stringify_variant(value));
    }

    stringified += "\n}";

    stringified
}

fn stringify_attributes(attributes: &HashMap<&str, Variant>, inst_name: &str) -> String {
    attributes.iter()
        .map(|(name, value)| { 
            format!(
                "\n{inst_name}:SetAttribute(\"{}\", {})",
                name, stringify_variant(value)
            )
        })
        .collect::<Vec<String>>()
        .concat()
}

fn stringify_style_rules_for_node(arena: &Arena<TokenTreeNode<'_>>, node: &TokenTreeNode, prev_counters: Option<&str>) -> String {
    let mut counter = -1;

    let mut stringified_rules: Vec<String> = vec![];

    for (selector, children) in &node.rules.0 {
        for child_idx in children {
            counter += 1;

            let this_counter = match prev_counters {
                Some(ref prev_counters) => &format!("{}{}", &prev_counters, counter),
                None => &counter.to_string()
            };

            let this_name = &format!("StyleRule_{}", this_counter);

            let node = arena.get(*child_idx).unwrap();

            let mut stringified_style_rule =
                format!("\n\nlocal {this_name} = Instance.new(\"StyleRule\")") +
                &format!("\n{this_name}.Selector = \"{}\"", selector);

            
            if let Some(priority) = &node.priority {
                stringified_style_rule += &format!("\n{this_name}.Priority = {}", priority);
            }

            stringified_style_rule += &format!("\n{this_name}.Name = \"{}\"", selector);

            let node_attributes = &node.variables;
            if !node_attributes.is_empty() {
                stringified_style_rule += &stringify_attributes(node_attributes, this_name)
            }

            let node_properties = &node.properties;
            if !node_properties.is_empty() {
                stringified_style_rule += &format!("\n{this_name}:SetProperties({})", stringify_properties(node_properties))
            }


            let parent = match prev_counters {
                Some(prev_counters) => &format!("StyleRule_{}", prev_counters),
                None => "StyleSheet"
            };
            stringified_style_rule += &format!("\n{this_name}.Parent = {}", parent);

            stringified_rules.push(stringified_style_rule);

            stringified_rules.push(stringify_style_rules_for_node(arena, node, Some(&this_counter)));
        }
    }

    stringified_rules.concat()
}

fn create_rsml_luau_file(event: Event) {
    if !matches!(
        event.kind,
        EventKind::Create(CreateKind::File) |
        EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content))
    ) { return }

    for path in event.paths {
        if path.extension() != Some(OsStr::new("rsml")) { continue; }

        let file_content = read_to_string(&path).unwrap();

        let tokens = lex_rsml(&file_content);
        let arena = parse_rsml(&tokens);

        let root_node = arena.get(0).unwrap();

        let mut source =
            "--!strict\n--!optimize 2\n--!nolint LocalShadow\n\n".to_string() +
            &"local StyleSheet = Instance.new(\"StyleSheet\")\n"+
            &format!("StyleSheet:AddTag(\"{}\")", string_to_ref(&stringify_path(&path))) +
            &stringify_style_rules_for_node(&arena, root_node, None);

        let node_attributes = &root_node.variables;
        if !node_attributes.is_empty() {
            source += &stringify_attributes(node_attributes, &"StyleSheet")
        }
        
        let derives = &root_node.derives;
        if !derives.is_empty() {
            source += "\n\nlocal CollectionService = game:GetService(\"CollectionService\")\ntask.spawn(function()\n    task.wait()";
            
            for derive in derives {
                let derive_path = Path::new(derive);

                let derive_path_string = match derive.starts_with("./") {
                    true => stringify_path(&path.join("..").join(derive_path)),
                    false => stringify_path(derive_path)
                };

                let derive_name = match &derive_path.file_stem() {
                    Some(file_stem) => match file_stem.to_str() {
                        Some(file) => &format!("{} (Derive)", file),
                        None => "StyleDerive"
                    },
                    None => "StyleDerive"
                };

                let derive_id = string_to_ref(&derive_path_string);

                source += &format!("\n\n    local ToDerive = CollectionService:GetTagged(\"{derive_id}\")[1]\n");
                source += "    if ToDerive and (typeof(ToDerive) == \"Instance\" and ToDerive.ClassName == \"StyleSheet\") then\n";
                source += "        local ThisStyleDerive = Instance.new(\"StyleDerive\")\n";
                source += "        ThisStyleDerive.StyleSheet = ToDerive :: StyleSheet\n";
                source += &format!("        ThisStyleDerive.Name = \"{derive_name}\"\n");
                source += "        ThisStyleDerive.Parent = StyleSheet\n";
                source += "    end";
            }

            source += "\nend)"
        }

        source += "\n\nStyleSheet.Parent = script";

        let mut output_path = path.clone();
        output_path.set_extension("rsml.client.luau");

        write(output_path, source);
    }
}

fn watch<P: AsRef<Path>>(path: P) -> notify::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    for res in rx {
        match res {
            Ok(event) => {
                //log::info!("Change: {event:?}");
                create_rsml_luau_file(event);
            },
            Err(error) => log::error!("Error: {error:?}"),
        }
    }

    Ok(())
}








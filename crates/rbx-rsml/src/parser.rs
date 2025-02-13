use crate::arena::Arena;
use crate::lexer::{DataType, Operator, TextType, Token};

use colors_transform::{Rgb, Color};
use rbx_types::{Color3, Font, FontStyle, FontWeight, Rect, UDim, UDim2, Variant, Vector2, Vector3};

use std::collections::HashMap;
use std::sync::LazyLock;

const TAILWIND_COLORS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_slice(include_bytes!("../tailwind_colors.json"))
        .expect("Could not read tailwind_colors.json file.")
});

const CSS_COLORS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_slice(include_bytes!("../css_colors.json"))
        .expect("Could not read css_colors.json file.")
});

const BRICK_COLORS: LazyLock<HashMap<String, [f32; 3]>> = LazyLock::new(|| {
    serde_json::from_slice(include_bytes!("../brick_colors.json"))
        .expect("Could not read css_colors.json file.")
});


#[derive(Clone, Debug)]
enum EquationDataType<'a> {
    NumberScale(f64),
    NumberOffset(f64),
    Number(f64),
    Operator(&'a Operator),
}

#[derive(Debug, Clone)]
struct TupleDataType<'a> {
    name: Option<&'a str>,
    data: Vec<DataType<'a>>,
    parent_idx: Option<usize>
}

impl<'a> TupleDataType<'a> {
    fn new(name: Option<&'a str>, parent_idx: Option<usize>) -> Self {
        Self {
            name,
            data: vec![],
            parent_idx
        }
    }

    fn push(&mut self, item: DataType<'a>) {
        self.data.push(item);
    }

    fn get(&self, idx: usize) -> Option<&DataType<'a>> {
        self.data.get(idx)
    }
}

#[derive(Debug)]
pub struct TokenTreeNodeRulesHashMap(pub HashMap<String, Vec<usize>>);

impl<'a> TokenTreeNodeRulesHashMap {
    fn new() -> Self {
        Self(HashMap::new())
    }

    fn insert(&mut self, selector: String, node_idx: usize) {
        let rules = self.0.entry(selector).or_insert(vec![]);

        rules.push(node_idx);
    }
}


#[derive(Debug)]
pub struct TokenTreeNodeMacrosHashMap<'a>(pub HashMap<&'a str, HashMap<Option<u64>, Vec<usize>>>);

impl<'a> TokenTreeNodeMacrosHashMap<'a> {
    fn new() -> Self {
        Self(HashMap::new())
    }

    /*fn insert(&mut self, macro_name: &'a str, args: Option<u64>, node_idx: usize) {
        let macro_hashmap = self.0.entry(macro_name).or_insert(HashMap::new());
        let args_hashmap = macro_hashmap.entry(args).or_insert(vec![]);

        args_hashmap.push(node_idx);
    }*/
}

#[derive(Debug)]
pub struct TokenTreeNode<'a> { 
    pub properties: HashMap<&'a str, Variant>,
    pub variables: HashMap<&'a str, Variant>,
    pub psuedo_properties: HashMap<&'a str, Variant>,
    pub rules: TokenTreeNodeRulesHashMap,
    pub macros: TokenTreeNodeMacrosHashMap<'a>,
    pub default_args: Option<(Variant,)>,
    pub priority: Option<i32>,
    pub derives: Vec<String>,
    pub parent_idx: usize
}

impl<'a> TokenTreeNode<'a> {
    fn new(parent_idx: usize) -> TokenTreeNode<'a> {
        TokenTreeNode {
            properties: HashMap::new(),
            variables: HashMap::new(),
            psuedo_properties: HashMap::new(),
            rules: TokenTreeNodeRulesHashMap::new(),
            macros: TokenTreeNodeMacrosHashMap::new(),
            default_args: None,
            derives: vec![],
            priority: None,
            parent_idx
        }
    }

    fn insert_rule(&mut self, selector: String, node_idx: usize) {
        self.rules.insert(selector, node_idx);
    }

    /*fn get_parent_from_arena<'b>(&'b self, arena: &'b Arena<TokenTreeNode<'b>>) -> &'b TokenTreeNode<'b> {
        arena.get(self.parent_idx).unwrap()
    }*/
}

struct Parser<'a> {
    tokens: &'a Vec<Token<'a>>,
    position: usize,

    tree_node_arena: Arena<TokenTreeNode<'a>>,
    current_tree_node_idx: usize,

    tuple_data_type_arena: Arena<TupleDataType<'a>>
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a Vec<Token<'a>>) -> Self {
        Self {
            tokens,
            position: 0,

            tree_node_arena: Arena::new(),
            current_tree_node_idx: 0,

            tuple_data_type_arena: Arena::new()
        }
    }

    fn get_token_at(&self, idx: usize) -> Option<&'a Token<'a>> {
        self.tokens.get(idx)
    }

    fn _raw_advance(&mut self) -> Option<&'a Token<'a>> {
        let advanced_position = self.position + 1;
        self.position = advanced_position;

        self.tokens.get(advanced_position)
    }

    fn advance(&mut self) -> Option<&'a Token<'a>> {
        self._raw_advance();

        if let Some(token) = self.get_token_at(self.position) { parse_multi_comment(token, self); }
        if let Some(token) = self.get_token_at(self.position) { parse_single_comment(token, self); }

        self.get_token_at(self.position)
    }

    fn peek(&mut self) -> Option<&'a Token<'a>> {
        self.tokens.get(self.position + 1)
    }


    fn add_tree_node(&mut self, node: TokenTreeNode<'a>) -> usize {
        let idx = self.tree_node_arena.push(node);
        self.current_tree_node_idx = idx;
        idx
    }

    fn get_tree_node_at(&mut self, idx: usize) -> Option<&TokenTreeNode<'_>> {
        self.tree_node_arena.get(idx)
    }

    fn get_mut_tree_node_at(&mut self, idx: usize) -> Option<&mut TokenTreeNode<'a>> {
        self.tree_node_arena.get_mut(idx)
    }

    fn backtrack(&self, backtrack_amount: usize) -> &'a [Token<'a>] {
        let parser_position = self.position;

        &self.tokens[parser_position-backtrack_amount..parser_position]
    }
}
// ---------------------------------------------------------------------------------------------------


// Parse Comments ------------------------------------------------------------------------------------
fn parse_single_comment<'a>(token: &Token, parser: &'a mut Parser) -> Option<&'a Token<'a>> {
    if matches!(token, Token::CommentSingle) {
        return parser.advance()
    };

    None
}

fn parse_multi_comment<'a>(token: &Token, parser: &'a mut Parser) -> Option<&'a Token<'a>>{
    if matches!(token, Token::CommentMultiStart) {
        loop {
            let token = parser.advance()?;

            if matches!(token, Token::CommentMultiEnd) {
                return parser.advance()
            }
        }
    }

    None
}
// ---------------------------------------------------------------------------------------------------


// Parse Priority ------------------------------------------------------------------------------------
fn parse_derive_value(token: &Token, parser: &mut Parser) -> Option<bool> {
    if let Token::DataType(DataType::StringSingle(value)) = token {
        let current_node = parser.get_mut_tree_node_at(parser.current_tree_node_idx).unwrap();
        current_node.derives.push(value.to_string());

        Some(true)
        
    } else {
        return Some(false)
    }
}

fn parse_derive<'a>(token: &Token, mut parser: &mut Parser) -> Option<bool> {
    if !matches!(token, Token::DeriveDeclaration) { return Some(false) }

    let next_token = parser.advance()?;
    if parse_derive_value(next_token, &mut parser)? { return Some(true) }

    Some(true)
}
// ---------------------------------------------------------------------------------------------------


// Parse Priority ------------------------------------------------------------------------------------
fn parse_priority_value(token: &Token, parser: &mut Parser) -> Option<bool> {
    if let Token::DataType(DataType::Number(value)) = token {
        let current_node = parser.get_mut_tree_node_at(parser.current_tree_node_idx).unwrap();
        current_node.priority = Some(value.round() as i32);

        Some(true)
        
    } else {
        return Some(false)
    }
}

fn parse_priority<'a>(token: &Token, mut parser: &mut Parser) -> Option<bool> {
    if !matches!(token, Token::PriorityDeclaration) { return Some(false) }

    let next_token = parser.advance()?;
    if parse_priority_value(next_token, &mut parser)? { return Some(true) }

    Some(true)
}
// ---------------------------------------------------------------------------------------------------


// Parse Scope ---------------------------------------------------------------------------------------
fn token_to_string_as_enum<'a>(token: &Token<'a>) -> String {
    match token {
        Token::EnumKeyword => "Enum".to_owned(),

        Token::Text(text_type) => match text_type {
            TextType::NonSpecial(text) |
            TextType::SelectorStateOrEnumPart(text) |
            TextType::SelectorTagOrEnumPart(text) |
            TextType::Variable(text) |
            TextType::PsuedoProperty(text) => String::from(*text),

            _ => String::from("")
        },

        _ => String::from("")
    }
}

fn text_type_to_string<'a>(text_type: &TextType) -> String {
    match text_type {
        TextType::NonSpecial(text) => String::from(*text),
        TextType::SelectorName(text) => format!("#{}", &text),
        TextType::SelectorTagOrEnumPart(text) => format!(".{}", &text),
        TextType::SelectorStateOrEnumPart(text) => format!(":{}", &text),
        TextType::SelectorPsuedo(text) => format!("::{}", &text),

        _ => String::from("")
    }
}

fn token_to_string<'a>(token: &Token<'a>) -> String {
    match token {
        Token::Text(text_type) => text_type_to_string(text_type),

        Token::ScopeToChildren => String::from(">"),

        Token::ScopeToDescendants => String::from(">>"),

        Token::ListDelimiter => String::from(","),

        Token::EnumKeyword => "Enum".to_owned(),

        _ => String::from("")
    }
}

fn parse_scope_name(token: &Token, parser: &mut Parser, mut backtrack_amount: usize) -> Option<bool> {
    if matches!(
        token, Token::Text(_) | Token::ScopeToChildren | Token::ScopeToDescendants | Token::ListDelimiter
    ) {
        let next_token = parser.advance();

        backtrack_amount += 1;

        if let Some(next_token) = next_token {
            if parse_scope_name(next_token, parser, backtrack_amount)? { return Some(true) }

        } 
    };

    if backtrack_amount == 0 { return Some(false) }

    if parse_scope_open(token, parser, backtrack_amount)? { return Some(true) }

    Some(false)
}

fn parse_scope_open(token: &Token, parser: &mut Parser, backtrack_amount: usize) -> Option<bool> {
    if !matches!(token, Token::ScopeOpen) { return Some(false) }

    let selector = parser.backtrack(backtrack_amount)
        .iter() 
        .map(token_to_string)
        .collect::<Vec<String>>();

    let old_node_idx = parser.current_tree_node_idx;
    let new_node_idx = parser.add_tree_node(TokenTreeNode::new(old_node_idx));

    let old_node = parser.get_mut_tree_node_at(old_node_idx).unwrap();
    old_node.insert_rule(selector.join(" "), new_node_idx);

    return Some(true)
}

fn parse_scope_close(token: &Token, parser: &mut Parser) -> Option<bool> {
    if !matches!(token, Token::ScopeClose) { return Some(false) }

    parser.current_tree_node_idx = parser.get_tree_node_at(parser.current_tree_node_idx).unwrap().parent_idx;

    return Some(true);
}


// Parse Tuple ---------------------------------------------------------------------------------------
fn tuple_to_vec2_data_type<'a>(tuple: &TupleDataType) -> DataType<'a> {
    let component_x = if let Some(component) = tuple.get(0) {
        match component {
            DataType::Number(number) => *number,
            _ => 0.0
        }
    } else { 0.0 };
    
    let component_y = if let Some(component) = tuple.get(1) {
        match component {
            DataType::Number(number) => *number,
            _ => component_x
        }
    } else { component_x };

    DataType::Vec2(Vector2::new(component_x as f32, component_y as f32))
}

fn tuple_to_vec3_data_type<'a>(tuple: &TupleDataType) -> DataType<'a> {
    let component_x = if let Some(component) = tuple.get(0) {
        match component {
            DataType::Number(number) => *number,
            _ => 0.0
        }
    } else { 0.0 };
    
    let component_y = if let Some(component) = tuple.get(1) {
        match component {
            DataType::Number(number) => *number,
            _ => 0.0
        }
    } else { 0.0 };

    let component_z = if let Some(component) = tuple.get(2) {
        match component {
            DataType::Number(number) => *number,
            _ => 0.0
        }
    } else { 0.0 };

    DataType::Vec3(Vector3::new(component_x as f32, component_y as f32, component_z as f32))
}

fn tuple_to_rect_data_type<'a>(tuple: &TupleDataType) -> DataType<'a> {
    let component_ax = if let Some(component) = tuple.get(0) {
        match component {
            DataType::Number(number) => *number,
            _ => 0.0
        }
    } else { 0.0 };
    
    let component_ay = if let Some(component) = tuple.get(1) {
        match component {
            DataType::Number(number) => *number,
            _ => 0.0
        }
    } else { 0.0 };

    let component_bx = if let Some(component) = tuple.get(2) {
        match component {
            DataType::Number(number) => *number,
            _ => 0.0
        }
    } else { 0.0 };

    let component_by = if let Some(component) = tuple.get(3) {
        match component {
            DataType::Number(number) => *number,
            _ => 0.0
        }
    } else { 0.0 };

    DataType::Rect(Rect::new(
        Vector2::new(component_ax as f32, component_ay as f32),
        Vector2::new(component_bx as f32, component_by as f32)
    ))
}

fn tuple_to_udim2_data_type<'a>(tuple: &TupleDataType) -> DataType<'a> {
    let component_x = if let Some(component) = tuple.get(0) {
        match component {
            DataType::UDim(udim) => *udim,
            DataType::Number(number) => UDim::new(*number as f32, 0),
            _ => UDim::new(0.0, 0)
        }
    } else { UDim::new(0.0, 0) };
    
    let component_y = if let Some(component) = tuple.get(1) {
        match component {
            DataType::UDim(udim) => *udim,
            DataType::Number(number) => UDim::new(*number as f32, 0),
            _ => component_x
        }
    } else { component_x };

    DataType::UDim2(UDim2::new(component_x, component_y))
}

fn tuple_to_color3_data_type<'a>(tuple: &TupleDataType) -> DataType<'a> {
    let component_r = if let Some(component) = tuple.get(0) {
        match component {
            DataType::Number(num) => *num,
            DataType::NumberScale(num) => *num,
            DataType::NumberOffset(num) => *num,
            DataType::UDim(udim) => udim.scale as f64,
            _ => 0.0,
        }
    } else { 0.0 };
    
    let component_g = if let Some(component) = tuple.get(1) {
        match component {
            DataType::Number(num) => *num,
            DataType::NumberScale(num) => *num,
            DataType::NumberOffset(num) => *num,
            DataType::UDim(udim) => udim.scale as f64,
            _ => 0.0,
        }
    } else { 0.0 };

    let component_b = if let Some(component) = tuple.get(2) {
        match component {
            DataType::Number(num) => *num,
            DataType::NumberScale(num) => *num,
            DataType::NumberOffset(num) => *num,
            DataType::UDim(udim) => udim.scale as f64,
            _ => 0.0,
        }
    } else { 0.0 };

    DataType::Color3(Color3::new(component_r as f32, component_g as f32, component_b as f32))
}

fn tuple_to_rgb_data_type<'a>(tuple: &TupleDataType) -> DataType<'a> {
    let color = tuple_to_color3_data_type(tuple);

    if let DataType::Color3(color) = color {
        return DataType::Color3(Color3::new(color.r / 255.0, color.g / 255.0, color.b / 255.0))
    } else { unreachable!() }
}

fn tuple_to_udim_data_type<'a>(tuple: &TupleDataType) -> DataType<'a> {
    let udim = if let Some(component) = tuple.get(0) {
        match component {
            DataType::Number(scale) => UDim::new(*scale as f32, 0),
            DataType::UDim(udim) => *udim,
            DataType::NumberScale(scale) => UDim::new(*scale as f32, 0),
            DataType::NumberOffset(offset) => UDim::new(0.0, *offset as i32),
            _ => UDim::new(0.0, 0)
        }
    } else { UDim::new(0.0, 0) };

    DataType::UDim(udim)
}

fn tuple_to_font_data_type<'a>(tuple: &TupleDataType) -> DataType<'a> {
    let font_name = if let Some(component) = tuple.get(0) {
        match component {
            DataType::StringSingle(str) => *str,
            DataType::Number(num) => &format!("rbxassetid://{}", num),
            _ => "rbxasset://fonts/families/SourceSansPro.json"
        }
    } else { "rbxasset://fonts/families/SourceSansPro.json" };

    let font_weight = if let Some(component) = tuple.get(1) {
        match component {
            DataType::StringSingle(str) => match *str {
                "Thin" => FontWeight::Thin,
                "ExtraLight" => FontWeight::ExtraLight,
                "Light" => FontWeight::Light,
                "Medium" => FontWeight::Medium,
                "SemiBold" => FontWeight::SemiBold,
                "Bold" => FontWeight::Bold,
                "ExtraBold" => FontWeight::ExtraBold,
                "Heavy" => FontWeight::Heavy,
                _ => FontWeight::Regular
            },
            _ => FontWeight::Regular
        }
    } else { FontWeight::Regular };

    let font_style = if let Some(component) = tuple.get(2) {
        match component {
            DataType::StringSingle(str) => match *str {
                "Italic" => FontStyle::Italic,
                _ => FontStyle::Normal
            },
            _ => FontStyle::Normal
        }
    } else { FontStyle::Normal };

    DataType::Font(Font::new(font_name, font_weight, font_style))
}

fn parse_tuple_as_number<'a>(tuple: &TupleDataType<'a>) -> Option<DataType<'a>> {
    if tuple.data.len() != 1 { return None }

    let data_type = tuple.get(0).unwrap();
    if matches!(data_type, DataType::Number(_)) { return Some(data_type.to_owned()) }

    None
}

fn parse_tuple_as_udim<'a>(tuple: &TupleDataType<'a>) -> Option<DataType<'a>> {
    if tuple.data.len() != 1 { return None }

    let data_type = tuple.get(0).unwrap();
    if matches!(data_type, DataType::UDim(_)) { return Some(data_type.to_owned()) }

    None
}

fn parse_tuple_as_udim2<'a>(tuple: &TupleDataType<'a>) -> Option<DataType<'a>> {
    if tuple.data.len() != 1 { return None }

    let data_type = tuple.get(0).unwrap();
    if matches!(data_type, DataType::UDim2(_)) { return Some(data_type.to_owned()) }

    None
}

fn tuple_to_data_type<'a>(tuple: &TupleDataType<'a>) -> Option<DataType<'a>> {
    if let Some (tuple_name) = tuple.name {
        match tuple_name {
            "udim2" => { return Some(tuple_to_udim2_data_type(tuple)) },
            "udim" => { return Some(tuple_to_udim_data_type(tuple)) },
            "vec2" => { return Some(tuple_to_vec2_data_type(tuple)) },
            "vec3" => { return Some(tuple_to_vec3_data_type(tuple)) },
            "rect" => { return Some(tuple_to_rect_data_type(tuple)) },
            "color3" => { return Some(tuple_to_color3_data_type(tuple)) },
            "rgb" => { return Some(tuple_to_rgb_data_type(tuple)) },
            "font" => { return Some(tuple_to_font_data_type(tuple)) },

            _ => { return None }
        };
    };

    if let Some(number) = parse_tuple_as_number(tuple) {
        return Some(number)
    }

    if let Some(udim) = parse_tuple_as_udim(tuple) {
        return Some(udim)
    }

    if let Some(udim2) = parse_tuple_as_udim2(tuple) {
        return Some(udim2)
    }

    None
}

fn parse_tuple_name<'a>(
    token: &'a Token, mut parser: &mut Parser<'a>, root_tuple_idx: Option<usize>, tuple_idx: Option<usize>, only_if_name: Option<&str>
) -> Option<usize> {
    let (tuple_name, next_token) = if let Token::Text(tuple_name) = token {
        let tuple_name: &'a str = match tuple_name {
            TextType::NonSpecial(text) => *text,
            _ => return None
        };

        if let Some(only_if_name) = only_if_name {
            if tuple_name != only_if_name {
                return None
            }
        }

        (Some(tuple_name), parser.advance()?)

    } else { (None, token) };

    if let Some(tuple_idx) = parse_tuple_open(next_token, &mut parser, tuple_name, root_tuple_idx, tuple_idx) {
        return Some(tuple_idx)
    };

    None
}

fn parse_tuple_open<'a>(
    token: &Token, mut parser: &mut Parser<'a>, tuple_name: Option<&'a str>, root_tuple_idx: Option<usize>, parent_tuple_idx: Option<usize>
) -> Option<usize> {
    if !matches!(token, Token::TupleOpen) { return None }

    let arena = &mut parser.tuple_data_type_arena;
    let tuple = TupleDataType::new(tuple_name, parent_tuple_idx);

    let tuple_idx = arena.push(tuple);
    let some_tuple_idx = Some(tuple_idx);

    let root_tuple_idx = root_tuple_idx.or(some_tuple_idx);

    let next_token = parser.advance()?;
  
    if let Some (tuple_idx) = parse_tuple_data_type(next_token, &mut parser, root_tuple_idx, some_tuple_idx) {
        return Some(tuple_idx)
    };
    if let Some(tuple_idx) = parse_tuple_delimiter(next_token, &mut parser, root_tuple_idx, some_tuple_idx) {
        return Some(tuple_idx)
    };
    if let Some(tuple_idx) = parse_tuple_close(next_token, &mut parser, root_tuple_idx, some_tuple_idx) {
        return Some(tuple_idx)
    };
    if let Some (tuple_idx) = parse_tuple_name(next_token, &mut parser, root_tuple_idx, some_tuple_idx, None) { return Some(tuple_idx) };

    None
}

fn parse_tuple_data_type<'a>(
    token: &'a Token, mut parser: &mut Parser<'a>, root_tuple_idx: Option<usize>, current_tuple_idx: Option<usize>
) -> Option<usize> {
    if let Some(data_type) = parse_data_type(token, parser, None) {
        let arena = &mut parser.tuple_data_type_arena;
        let tuple = arena.get_mut(current_tuple_idx.unwrap()).unwrap();

        tuple.push(data_type);

        let next_token = parser.advance()?;

        if let Some(tuple_idx) = parse_tuple_delimiter(next_token, &mut parser, root_tuple_idx, current_tuple_idx) { return Some(tuple_idx) };
        if let Some(tuple_idx) = parse_tuple_data_type(next_token, &mut parser, root_tuple_idx, current_tuple_idx) { return Some(tuple_idx) };
        if let Some(tuple_idx) = parse_tuple_close(next_token, &mut parser, root_tuple_idx, current_tuple_idx) { return Some(tuple_idx) };
        if let Some(tuple_idx) = parse_tuple_name(next_token, &mut parser, root_tuple_idx, current_tuple_idx, None) {
            return Some(tuple_idx)
        };
    };

    return None
}

fn parse_tuple_delimiter<'a>(
    token: &Token, mut parser: &mut Parser<'a>, root_tuple_idx: Option<usize>, current_tuple_idx: Option<usize>
) -> Option<usize> {
    if !matches!(token, Token::ListDelimiter | Token::SectionClose) { return None }

    let next_token = parser.advance()?;

    // Handles cases where there are multiple delimiter tokens next to each other.
    if let Some(tuple_idx) = parse_tuple_delimiter(next_token, &mut parser, root_tuple_idx, current_tuple_idx) { return Some(tuple_idx) };

    if let Some(tuple_idx) = parse_tuple_data_type(next_token, &mut parser, root_tuple_idx, current_tuple_idx) { return Some(tuple_idx) };
    if let Some(tuple_idx) = parse_tuple_close(next_token, &mut parser, root_tuple_idx, current_tuple_idx) { return Some(tuple_idx) };
    if let Some(tuple_idx) = parse_tuple_name(next_token, &mut parser, root_tuple_idx, current_tuple_idx, None) {
        return Some(tuple_idx)
    };

    None
}

fn parse_tuple_close<'a>(
    token: &Token, mut parser: &mut Parser<'a>, root_tuple_idx: Option<usize>, current_tuple_idx: Option<usize>
) -> Option<usize> {
    if !matches!(token, Token::TupleClose) { return None }

    let arena = &mut parser.tuple_data_type_arena;
    let tuple = arena.get_mut(current_tuple_idx.unwrap()).unwrap();
    let parent_tuple_idx = tuple.parent_idx;

    if let Some(parent_tuple_idx) = parent_tuple_idx {
        let data_type = tuple_to_data_type(tuple);
        let parent_tuple = arena.get_mut(parent_tuple_idx).unwrap();

        if let Some(data_type) = data_type {
            parent_tuple.push(data_type);
        }
    }

    // We are at the root of the tuple, no need to parse more tuple tokens.
    if root_tuple_idx == current_tuple_idx {
        return root_tuple_idx;
    }

    let parent_tuple_idx_with_fallback = parent_tuple_idx.or(current_tuple_idx);

    let next_token = parser.advance()?;

    if let Some(tuple_idx) = parse_tuple_data_type(next_token, &mut parser, root_tuple_idx, parent_tuple_idx_with_fallback) {
        return Some(tuple_idx)
    };
    if let Some(tuple_idx) = parse_tuple_delimiter(next_token, &mut parser, root_tuple_idx, parent_tuple_idx_with_fallback) {
        return Some(tuple_idx)
    };
    if let Some(tuple_idx) = parse_tuple_close(next_token, &mut parser, root_tuple_idx, parent_tuple_idx_with_fallback) {
        return Some(tuple_idx)
    };
    if let Some(tuple_idx) = parse_tuple_name(next_token, &mut parser, root_tuple_idx, parent_tuple_idx_with_fallback, None) {
        return Some(tuple_idx)
    };

    None
}
// ---------------------------------------------------------------------------------------------------


// Parse Data Types ----------------------------------------------------------------------------------
fn will_divide_by_zero(a: f64, b: f64) -> bool {
    if a != 0.0 && b != 0.0 { return false }
    return true
}

fn pos_f64(a: f64) -> f64 { a }
fn neg_f64(a: f64) -> f64 { -a }

fn multiply_f64(a: f64, b: f64) -> f64 { a * b }
fn divide_f64(a: f64, b: f64) -> f64 {
    if will_divide_by_zero(a, b) { return a }
    a / b
}
fn power_f64(a: f64, b: f64) -> f64 { a.powf(b) }
fn modulo_f64(a: f64, b: f64) -> f64 { a % b }

fn operator_indexes_in_stack<'a>(stack: &Vec<EquationDataType<'a>>, operator: &Operator) -> Vec<usize> {
    let mut indexes: Vec<usize> = vec![];

    for (idx, item) in stack.iter().enumerate() {
        match item {
            EquationDataType::Operator(current_operator) => {
                if *current_operator == operator { indexes.push(idx) }
            },

            _ => ()
        }
    }

    indexes
}

fn resolve_equation_stack<'a>(stack: &mut Vec<EquationDataType<'a>>) -> DataType<'a> {
    for (operator, operator_fn) in [
        (Operator::Pow, power_f64 as fn(f64, f64) -> f64),
        (Operator::Div, divide_f64 as fn(f64, f64) -> f64),
        (Operator::Mod, modulo_f64 as fn(f64, f64) -> f64),
        (Operator::Mult, multiply_f64 as fn(f64, f64) -> f64),
    ] {
        let occurrences = operator_indexes_in_stack(stack, &operator);
        let mut stack_offset: usize = 0;

        for occurrence_idx in occurrences {
            let mut occurrence_idx =  occurrence_idx - stack_offset;

            let right_idx = occurrence_idx + 1;
            if right_idx > stack.len() { continue; }

            stack_offset += 1;
            let right = stack.remove(right_idx);

            // If the operation is the first item of the array then we assign `0` to the left side.
            let left = if occurrence_idx == 0 {
                EquationDataType::Number(0.0)
            } else {
                let left_idx = occurrence_idx - 1;

                stack_offset += 1;
                occurrence_idx -= 1;
                stack.remove(left_idx)
            };

            let right_value = if let EquationDataType::Number(num) 
                | EquationDataType::NumberScale(num) 
                | EquationDataType::NumberOffset(num) = right { num } else { unreachable!() };

             match left {
                // If the left side is a scale then the result is a scale.
                EquationDataType::NumberScale(left_value)  => {
                    stack[occurrence_idx] = EquationDataType::NumberScale(operator_fn(left_value, right_value));
                },

                // If the left side is an offset then the result is a scale.
                EquationDataType::NumberOffset(left_value) => {
                    stack[occurrence_idx] = EquationDataType::NumberOffset(operator_fn(left_value, right_value));
                },

                // If the left side doesn't have an explicit measurement type then we need to
                // attempt to get it from the right side instead.
                EquationDataType::Number(left_value) => match right {
                    // If the right side is a scale then the result is a scale.
                    EquationDataType::NumberScale(right_value) => {
                        stack[occurrence_idx] = EquationDataType::NumberScale(operator_fn(left_value, right_value));
                    },

                    // If the right side is a scale then the result is an offset.
                    EquationDataType::NumberOffset(right_value) => {
                        stack[occurrence_idx] = EquationDataType::NumberOffset(operator_fn(left_value, right_value));
                    },

                    // If the right side doesn't have an explicit measurement type either then the result is a number.
                    EquationDataType::Number(right_value) => {
                        stack[occurrence_idx] = EquationDataType::Number(operator_fn(left_value, right_value));
                    },

                    _ => ()
                }

                _ => ()
            };
        }
    }

    for (operator, operator_fn) in [
        (Operator::Plus, pos_f64 as fn(f64) -> f64),
        (Operator::Sub, neg_f64 as fn(f64) -> f64),
    ] {
        let occurrences = operator_indexes_in_stack(stack, &operator);
        let mut stack_offset: usize = 0;

        for occurrence_idx in occurrences {
            let occurrence_idx = occurrence_idx - stack_offset;

            let right_idx = occurrence_idx + 1;
            if right_idx > stack.len() { continue; }

            stack_offset += 1;
            let right = stack.remove(right_idx);

            match right {
                EquationDataType::Number(right_value) => {
                    stack[occurrence_idx] = EquationDataType::Number(operator_fn(right_value));
                },
                EquationDataType::NumberScale(right_value) => {
                    stack[occurrence_idx] = EquationDataType::NumberScale(operator_fn(right_value));
                },
                EquationDataType::NumberOffset(right_value) => {
                    stack[occurrence_idx] = EquationDataType::NumberOffset(operator_fn(right_value));
                },
                _ => ()
            }
        }
    }

    let (mut scale, mut offset) = (0.0_f64, 0);
    let mut has_explicit_scale = false;
    let mut has_explicit_offset = false;

    for number in stack {
        match number {
            EquationDataType::Number(value) => { scale += *value },
            EquationDataType::NumberScale(value) => {
                has_explicit_scale = true;
                scale += *value
            },
            EquationDataType::NumberOffset(value) => {
                has_explicit_offset = true;
                offset += *value as i32
            },
            _ => ()
        }
    }

    if !has_explicit_scale && !has_explicit_offset {
        return DataType::Number(scale)

    } else { return DataType::UDim(UDim::new(scale as f32, offset)) }
}

fn previous_token_operator<'a>(stack: &mut Vec<EquationDataType<'a>>) -> Option<&'a Operator> {
    let stack_len = stack.len();
    if stack_len == 0 { return None }
    
    let previous_token = stack.get(stack.len() - 1).unwrap();

    if let EquationDataType::Operator(operator) = previous_token {
        Some(operator)
    } else { Some(&Operator::Mult) }
}

fn parse_equation_tuple_data_type<'a>(
    token: &'a Token, parser: &mut Parser<'a>, stack: &mut Vec<EquationDataType<'a>>, only_if_name: Option<&str>
) -> Option<usize> {
    if let Some(tuple_idx) = parse_tuple_name(token, parser,None, None, only_if_name) {
        let tuple_data_type = tuple_to_data_type(parser.tuple_data_type_arena.get(tuple_idx).unwrap());
        
        if let Some(tuple_data_type) = tuple_data_type {
            match tuple_data_type {
                DataType::UDim(udim) => {
                    let (scale, offset) = (udim.scale, udim.offset);

                    let previous_token_operator = previous_token_operator(stack);

                    let (first_operator, second_operator) = {
                        match previous_token_operator {
                            None => {
                                (None, EquationDataType::Operator(&Operator::Plus))
                            },

                            Some(operator) => (None, EquationDataType::Operator(operator))
                        }
                    };

                    let apply_scale = scale != 0.0;
                    if apply_scale {
                        if let Some(first_operator) = first_operator.clone() {
                            stack.push(first_operator);
                        } 
                        stack.push(EquationDataType::NumberScale(udim.scale as f64));
                    }

                    if offset != 0 {
                        if apply_scale {
                            stack.push(second_operator);

                        } else {
                            if let Some(first_operator) = first_operator {
                                stack.push(first_operator);
                            }
                        };

                        stack.push(EquationDataType::NumberOffset(udim.offset as f64));
                    }
                },

                DataType::Number(number) => {
                    stack.push(EquationDataType::Number(number as f64));
                },

                _ => ()
            }
        }

        return Some(tuple_idx)
    }

    None
}

fn parse_equation_latest_operator_data_type<'a>(
    current_operator: &'a Operator, parser: &mut Parser<'a>, stack: &mut Vec<EquationDataType<'a>>
) -> Option<bool> {
    let next_token = parser.peek()?;

    if let Token::Operator(next_operator) = next_token {
        parser.advance();

        let next_operator = match (current_operator, next_operator) {
            (Operator::Sub, Operator::Plus) => &Operator::Sub,
            (Operator::Sub, Operator::Sub) => &Operator::Plus,
            _ => next_operator
        };

        return parse_equation_latest_operator_data_type(next_operator, parser, stack);
    } else {
        stack.push(EquationDataType::Operator(current_operator));
    }

    return Some(true)
}

fn parse_equation_data_types<'a>(token: &'a Token, parser: &mut Parser<'a>, stack: &mut Vec<EquationDataType<'a>>) -> Option<DataType<'a>> {
    match token {
        Token::DataType(data_type) => match data_type {
            DataType::Number(num) => stack.push(EquationDataType::Number(*num)),
            DataType::NumberOffset(num) => stack.push(EquationDataType::NumberOffset(*num)),
            DataType::NumberScale(num) => stack.push(EquationDataType::NumberScale(*num)),
            _ => { return None }
        },

        Token::Operator(operator) => { parse_equation_latest_operator_data_type(operator, parser, stack); },

        _ => if parse_equation_tuple_data_type(token, parser, stack, Some("udim")).is_none() { return None }
    };

    let next_token = parser.advance()?;

    if let Some(result) = parse_equation_data_types(next_token, parser, stack) { return Some(result) };


    parser.position -= 1;
    let solved = resolve_equation_stack(stack);
    return Some(solved)
}
// ---------------------------------------------------------------------------------------------------


// Parse Assignment ----------------------------------------------------------------------------------
fn data_type_to_variant(data_type: &DataType) -> Variant {
    match data_type {
        DataType::StringSingle(data_type) => Variant::String(data_type.to_string()),
        DataType::OwnedString(data_type) => Variant::String(data_type.to_owned()),
        DataType::UDim(data_type) => Variant::UDim(*data_type),
        DataType::UDim2(data_type) => Variant::UDim2(*data_type),
        DataType::Vec2(data_type) => Variant::Vector2(*data_type),
        DataType::Bool(data_type) => Variant::Bool(*data_type),
        DataType::Vec3(data_type) => Variant::Vector3(*data_type),
        DataType::Color3(data_type) => Variant::Color3(*data_type),
        DataType::Number(data_type) => Variant::Float32(*data_type as f32),
        DataType::NumberOffset(data_type) => Variant::UDim(UDim::new(0.0, *data_type as i32)),
        DataType::NumberScale(data_type) => Variant::UDim(UDim::new(*data_type as f32, 0)),
        DataType::Font(font) => Variant::Font(font.clone()),
        DataType::Rect(rect) => Variant::Rect(rect.clone()),

        _ => Variant::String(format!("{:#?}", "placeholder"))
    }
}

fn parse_hex<'a>(hex_str: &str) -> DataType<'a> {
    let rgb = match Rgb::from_hex_str(hex_str) {
        Ok(ok_rgb) => ok_rgb,
        Err(_) => Rgb::from(255.0, 0.0, 249.0)
    };

    DataType::Color3(Color3::new(rgb.get_red() / 255.0, rgb.get_green() / 255.0, rgb.get_blue() / 255.0))
}

fn parse_hex_data_type<'a>(token: &'a Token) -> Option<DataType<'a>> {
    if let Token::DataType(DataType::ColorHex(hex_color)) = token {
        return Some(parse_hex(hex_color))
    }

    None
}

fn parse_tailwind_color_data_type<'a>(token: &'a Token) -> Option<DataType<'a>> {
    if let Token::DataType(DataType::ColorTw(tailwind_color)) = token {
        return Some(parse_hex(TAILWIND_COLORS.get(tailwind_color.to_owned()).unwrap()))
    }

    None
}

fn parse_css_color_data_type<'a>(token: &'a Token) -> Option<DataType<'a>> {
    if let Token::DataType(DataType::ColorCss(css_color)) = token {
        return Some(parse_hex(CSS_COLORS.get(css_color.to_owned()).unwrap()))
    }

    None
}

fn parse_brick_color_data_type<'a>(token: &'a Token) -> Option<DataType<'a>> {
    if let Token::DataType(DataType::ColorBc(brick_color_name)) = token {
        let brick_color = BRICK_COLORS.get(brick_color_name.to_owned()).unwrap().to_owned();

        return Some(
            DataType::Color3(Color3::new(brick_color[0] / 255.0, brick_color[1] / 255.0, brick_color[2] / 255.0))
        )
    }

    None
}

fn parse_full_enum<'a>(
    token: &'a Token, parser: &mut Parser<'a>, key: &'a TextType<'_>, mut backtrack_amount: usize
) -> Option<DataType<'a>> {
    if matches!(
        token, Token::Text(TextType::SelectorStateOrEnumPart(_) | TextType::SelectorTagOrEnumPart(_))
    ) {
        backtrack_amount += 1;

        let next_token = parser.advance();

        if let Some(next_token) = next_token {
            if let Some(enum_data_type) = parse_full_enum(next_token, parser, key, backtrack_amount) {
                return Some(enum_data_type)
            }
        }
    };

    if backtrack_amount == 0 { return None }

    let enum_data_type = parser.backtrack(backtrack_amount).to_vec();

    Some(DataType::OwnedString(enum_data_type
        .iter()
        .map(token_to_string_as_enum)
        .collect::<Vec<String>>()
        .join(".")
    ))
}

fn parse_enum_keyword<'a>(
    token: &'a Token, parser: &mut Parser<'a>, key: &'a TextType<'_>
) -> Option<DataType<'a>> {
    if matches!(token, Token::EnumKeyword) {
        let next_token = parser.advance()?;

        return parse_full_enum(next_token, parser, key, 1)
    } else {
        return None
    }
}

fn parse_enum_shorthand<'a>(
    token: &'a Token, key: &'a TextType<'_>
) -> Option<DataType<'a>> {
    match token {
        Token::Text(
            TextType::SelectorStateOrEnumPart(value) | TextType::SelectorTagOrEnumPart(value)
        ) => Some(DataType::OwnedString(format!("Enum.{}.{}", text_type_to_string(key), &value))),

        _ => None
    }
}

fn parse_enum_data_type<'a>(
    token: &'a Token, parser: &mut Parser<'a>, key: Option<&'a TextType<'_>>
) -> Option<DataType<'a>> {
    if let Some(key) = key {
        if let Some(enum_datatype) = parse_enum_keyword(token, parser, key) { return Some(enum_datatype) }
        if let Some(enum_datatype) = parse_enum_shorthand(token, key) { return Some(enum_datatype) }
    }  

    None
}

fn parse_data_type<'a>(token: &'a Token, parser: &mut Parser<'a>, key: Option<&'a TextType<'_>>) -> Option<DataType<'a>> {
    if let Token::Text(TextType::Variable(text)) = token {
        Some(DataType::OwnedString(format!("${}", text)))

    } else if let Some(data_type) = parse_equation_data_types(token, parser, &mut vec![]) {
        Some(data_type)

    } else if let Some(tuple_idx) = parse_tuple_name(token, parser, None, None, None) {
        if let Some(data_type) = tuple_to_data_type(parser.tuple_data_type_arena.get(tuple_idx).unwrap()) {
            Some(data_type)
        } else {
            return None
        }

    } else if let Some(enum_data_type) = parse_enum_data_type(token, parser, key) {
        Some(enum_data_type)
    

    } else if let Some(hex_data_type) = parse_hex_data_type(token) {
        Some(hex_data_type)

    } else if let Some(tailwind_color_data_type) = parse_tailwind_color_data_type(token) {
        Some(tailwind_color_data_type)

    } else if let Some(css_color_data_type) = parse_css_color_data_type(token) {
        Some(css_color_data_type)

    } else if let Some(css_color_data_type) = parse_brick_color_data_type(token) {
            Some(css_color_data_type)
    
    } else if let Token::DataType(data_type_value) = token {
        Some(data_type_value.to_owned())
    
    } else {
        return None
    }
}

fn parse_assignment<'a>(token: &'a Token, parser: &mut Parser<'a>, key: &'a TextType) -> Option<bool> {
    if let Some(data_type) = parse_data_type(token, parser, Some(key)) {
        let variant = data_type_to_variant(&data_type);

        let current_node = parser.get_mut_tree_node_at(parser.current_tree_node_idx).unwrap();
    
        match key {
            TextType::NonSpecial(key) => current_node.properties.insert(key, variant),
            TextType::Variable(key) => current_node.variables.insert(key, variant),
            TextType::PsuedoProperty(key) => current_node.psuedo_properties.insert(key, variant),
    
            _ => None
        };

        return Some(true)
    };

    Some(false)
}

fn parse_assignment_equals<'a>(token: &Token, mut parser: &mut Parser<'a>, key: &'a TextType) -> Option<bool> {
    if !matches!(token, Token::Equals) { return Some(false) }

    let next_token = parser.advance()?;
    if parse_assignment(next_token, &mut parser, key)? { return Some(true) }

    Some(false)
}
// ---------------------------------------------------------------------------------------------------


// Parse Text ----------------------------------------------------------------------------------------
fn parse_text<'a>(token: &'a Token, mut parser: &mut Parser<'a>) -> Option<bool> {
    if let Token::Text(text) = token {
        let next_token = parser.advance()?;
        if parse_assignment_equals(next_token, &mut parser, text)? { return Some(true) }
        if parse_scope_name(next_token, &mut parser, 1)? { return Some(true) }
    }

    Some(false)  
}
// ---------------------------------------------------------------------------------------------------


pub fn parse_rsml<'a>(tokens: &'a Vec<Token>) -> Arena<TokenTreeNode<'a>> {
    let mut parser = Parser::new(tokens);

    let root_node = TokenTreeNode::new(0);
    parser.add_tree_node(root_node);

    let tokens_len = tokens.len();

    if tokens_len == 0 { return parser.tree_node_arena }

    loop {

        if let Some(token) = parser.get_token_at(parser.position) { parse_text(token, &mut parser); }
        if let Some(token) = parser.get_token_at(parser.position) { parse_scope_name(token, &mut parser, 0); }
        if let Some(token) = parser.get_token_at(parser.position) { parse_priority(token, &mut parser); }
        if let Some(token) = parser.get_token_at(parser.position) { parse_derive(token, &mut parser); }
        if let Some(token) = parser.get_token_at(parser.position) { parse_scope_close(token, &mut parser); }

        if parser.advance().is_none() { break }
    }

    parser.tree_node_arena
}
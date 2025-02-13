use logos::Logos;
use rbx_types::{Color3, Font, Rect, UDim, UDim2, Vector2, Vector3};

#[derive(Hash, Eq, Debug, PartialEq, Clone)]
pub enum TextType<'a> {
    NonSpecial(&'a str),
    SelectorName(&'a str),
    SelectorTagOrEnumPart(&'a str),
    SelectorStateOrEnumPart(&'a str),
    SelectorPsuedo(&'a str),
    Argument(&'a str),
    Variable(&'a str),
    PsuedoProperty(&'a str)
}

#[derive(Debug, PartialEq, Clone)]
pub enum DataType<'a> {
    ColorHex(&'a str),
    ColorTw(&'a str),
    ColorCss(&'a str),
    ColorBc(&'a str),
    StringSingle(&'a str),
    NumberOffset(f64),
    NumberScale(f64),
    Number(f64),
    Bool(bool),

    Tuple(usize),
    UDim(UDim),
    UDim2(UDim2),
    Vec2(Vector2),
    Rect(Rect),
    Vec3(Vector3),
    Color3(Color3),
    Font(Font),
    OwnedString(String)
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Operator {
    Plus,
    Sub,
    Mult,
    Div,
    Pow,
    Mod,
}

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[\n\f\r ]+")] // Ignore this regex pattern between tokens
pub enum Token<'a> {
    #[token("--[[", priority = 99)]
    CommentMultiStart,
    #[token(r"]]", priority = 99)]
    CommentMultiEnd,

    #[regex(r"-- *[^\[\n\f\r]+", priority = 98)]
    CommentSingle,

    #[regex(r#"[a-zA-Z0-9"'_-]+"#, |lex| TextType::NonSpecial(lex.slice()), priority = 1)]
    #[regex(r#"#[a-zA-Z0-9"'_-]+"#, |lex| TextType::SelectorName(str_clip(lex.slice(), 1, 0)), priority = 1)]
    #[regex(r#"\.[a-zA-Z0-9"'_-]+"#, |lex| TextType::SelectorTagOrEnumPart(str_clip(lex.slice(), 1, 0)), priority = 1)]
    #[regex(r#":[a-zA-Z0-9"'_-]+"#, |lex| TextType::SelectorStateOrEnumPart(str_clip(lex.slice(), 1, 0)), priority = 1)]
    #[regex(r#"::[a-zA-Z0-9"'_-]+"#, |lex| TextType::SelectorPsuedo(str_clip(lex.slice(), 2, 0)), priority = 1)]
    #[regex(r#"\$![a-zA-Z0-9"'_-]+"#, |lex| TextType::Argument(str_clip(lex.slice(), 2, 0)), priority = 1)]
    #[regex(r#"\$[a-zA-Z0-9"'_-]+"#, |lex| TextType::Variable(str_clip(lex.slice(), 1, 0)), priority = 1)]
    #[regex(r#"![a-zA-Z0-9"'_-]+"#, |lex| TextType::PsuedoProperty(str_clip(lex.slice(), 1, 0)), priority = 1)]
    Text(TextType<'a>),

    #[token("Enum")]
    EnumKeyword,

    #[regex(r"tw:(slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)(:(950|900|800|700|600|500|400|300|200|100|50))?", |lex| DataType::ColorTw(lex.slice()), priority = 2)]
    #[regex(r"css:(aliceblue|antiquewhite|aqua|aquamarine|azure|beige|bisque|black|blanchedalmond|blue|blueviolet|brown|burlywood|cadetblue|chartreuse|chocolate|coral|cornflowerblue|cornsilk|crimson|cyan|darkblue|darkcyan|darkgoldenrod|darkgray|darkgreen|darkgrey|darkkhaki|darkmagenta|darkolivegreen|darkorange|darkorchid|darkred|darksalmon|darkseagreen|darkslateblue|darkslategray|darkslategrey|darkturquoise|darkviolet|deeppink|deepskyblue|dimgray|dimgrey|dodgerblue|firebrick|floralwhite|forestgreen|fuchsia|gainsboro|ghostwhite|goldenrod|gold|gray|green|greenyellow|grey|honeydew|hotpink|indianred|indigo|ivory|khaki|lavenderblush|lavender|lawngreen|lemonchiffon|lightblue|lightcoral|lightcyan|lightgoldenrodyellow|lightgray|lightgreen|lightgrey|lightpink|lightsalmon|lightseagreen|lightskyblue|lightslategray|lightslategrey|lightsteelblue|lightyellow|lime|limegreen|linen|magenta|maroon|mediumaquamarine|mediumblue|mediumorchid|mediumpurple|mediumseagreen|mediumslateblue|mediumspringgreen|mediumturquoise|mediumvioletred|midnightblue|mintcream|mistyrose|moccasin|navajowhite|navy|oldlace|olive|olivedrab|orange|orangered|orchid|palegoldenrod|palegreen|paleturquoise|palevioletred|papayawhip|peachpuff|peru|pink|plum|powderblue|purple|rebeccapurple|red|rosybrown|royalblue|saddlebrown|salmon|sandybrown|seagreen|seashell|sienna|silver|skyblue|slateblue|slategray|slategrey|snow|springgreen|steelblue|tan|teal|thistle|tomato|turquoise|violet|wheat|white|whitesmoke|yellow|yellowgreen)", |lex| DataType::ColorCss(lex.slice()), priority = 2)]
    #[regex(r"bc:(white|grey|lightyellow|brickyellow|lightgreen|lightreddishviolet|pastelblue|lightorangebrown|nougat|brightred|medreddishviolet|brightblue|brightyellow|earthorange|black|darkgrey|darkgreen|mediumgreen|ligyellowichorange|brightgreen|darkorange|lightbluishviolet|transparent|trred|trlgblue|trblue|tryellow|lightblue|trflureddishorange|trgreen|trflugreen|phosphwhite|lightred|mediumred|mediumblue|lightgrey|brightviolet|bryellowishorange|brightorange|brightbluishgreen|earthyellow|brightbluishviolet|trbrown|mediumbluishviolet|trmedireddishviolet|medyellowishgreen|medbluishgreen|lightbluishgreen|bryellowishgreen|ligyellowishgreen|medyellowishorange|brreddishorange|brightreddishviolet|lightorange|trbrightbluishviolet|darknougat|silver|neonorange|neongreen|sandblue|sandviolet|mediumorange|sandyellow|earthblue|earthgreen|trflublue|sandbluemetallic|sandvioletmetallic|sandyellowmetallic|darkgreymetallic|blackmetallic|lightgreymetallic|sandgreen|sandred|darkred|trfluyellow|trflured|gunmetallic|redflipflop|yellowflipflop|silverflipflop|curry|fireyellow|flameyellowishorange|reddishbrown|flamereddishorange|mediumstonegrey|royalblue|darkroyalblue|brightreddishlilac|darkstonegrey|lemonmetalic|lightstonegrey|darkcurry|fadedgreen|turquoise|lightroyalblue|mediumroyalblue|brown|reddishlilac|lightlilac|brightpurple|lightpurple|lightpink|lightbrickyellow|warmyellowishorange|coolyellow|doveblue|mediumlilac|slimegreen|smokygrey|darkblue|parsleygreen|steelblue|stormblue|lapis|darkindigo|seagreen|shamrock|fossil|mulberry|forestgreen|cadetblue|electricblue|eggplant|moss|artichoke|sagegreen|ghostgrey|lilac|plum|olivine|laurelgreen|quillgrey|crimson|mint|babyblue|carnationpink|persimmon|maroon|gold|daisyorange|pearl|fog|salmon|terracotta|cocoa|wheat|buttermilk|mauve|sunrise|tawny|rust|cashmere|khaki|lilywhite|seashell|burgundy|cork|burlap|beige|oyster|pinecone|fawnbrown|hurricanegrey|cloudygrey|linen|copper|mediumbrown|bronze|flint|darktaupe|burntsienna|institutionalwhite|midgray|reallyblack|reallyred|deeporange|alder|dustyrose|olive|newyeller|reallyblue|navyblue|deepblue|cyan|cgabrown|magenta|pink|teal|toothpaste|limegreen|camo|grime|lavender|pastellightblue|pastelorange|pastelviolet|pastelbluegreen|pastelgreen|pastelyellow|pastelbrown|royalpurple|hotpink)", |lex| DataType::ColorBc(lex.slice()), priority = 2)]
    #[regex(r"#[0-9a-fA-F]+", |lex| DataType::ColorHex(lex.slice()))]
    #[regex(r#"'([^'\n\f\r])*'"#, |lex| DataType::StringSingle(str_clip(lex.slice(), 1, 1)))]
    #[regex(r#""([^"\n\f\r])*""#, |lex| DataType::StringSingle(str_clip(lex.slice(), 1, 1)))]
    #[regex(r"[+-]?([0-9]+([.][0-9]*)?|[.][0-9]+)px", |lex| DataType::NumberOffset(match str_clip(lex.slice(), 0, 2).parse::<f64>() {
        Ok(float) => float,
        Err(_) => 0.0 
    }))]
    #[regex(r"[+-]?([0-9]+([.][0-9]*)?|[.][0-9]+)%", |lex| DataType::NumberScale(match str_clip(lex.slice(), 0, 1).parse::<f64>() {
        Ok(float) => float / 100.0,
        Err(_) => 0.0
    }))]
    #[regex(r"[+-]?([0-9]+([.][0-9]*)?|[.][0-9]+)", |lex| DataType::Number(match lex.slice().parse::<f64>() {
        Ok(float) => float,
        Err(_) => 0.0
    }))]
    #[regex(r"rbxassetid://[0-9]+", |lex| DataType::StringSingle(lex.slice()))]

    #[token("true", |_| DataType::Bool(true))]
    #[token("false", |_| DataType::Bool(false))]
    DataType(DataType<'a>),

    #[token("+", |_| Operator::Plus)]
    #[token("-", |_| Operator::Sub)]
    #[token("*", |_| Operator::Mult)]
    #[token("/", |_| Operator::Div)]
    #[token("^", |_| Operator::Pow)]
    #[token("%", |_| Operator::Mod)]
    Operator(Operator),

    #[token("{")]
    ScopeOpen,

    #[token("}")]
    ScopeClose,

    #[token(";")]
    SectionClose,

    #[token(",")]
    ListDelimiter,

    #[token("=")]
    Equals,

    #[token(":")]
    Colon,

    #[token(">")]
    ScopeToChildren,

    #[token(">>")]
    ScopeToDescendants,

    #[token("(")]
    TupleOpen,

    #[token(")")]
    TupleClose,

    #[token("@macro")]
    MacroDeclaration,

    #[token("@priority")]
    PriorityDeclaration,

    #[token("@derive")]
    DeriveDeclaration
}

pub type RsmlLexer<'a> = logos::Lexer<'a, Token<'a>>;

fn str_clip(str: &str, start: usize, end: usize) -> &str {
    &str[start..str.len() - end]
}

pub fn lex_rsml(source: &str) -> Vec<Token<'_>> {
    Token::lexer(&source).filter_map(|token| {
        if token.is_ok() {
            return Some(token.unwrap())
        }
        None
    }).collect()
}

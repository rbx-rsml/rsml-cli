use rbx_rsml::{lex_rsml, parse_rsml};

use std::fs;


fn main() {
    let source = &fs::read_to_string("./src/styles.rsml").unwrap();

    let tokens = lex_rsml(source);
    println!("{:#?}", tokens);

    let nodes = parse_rsml(&tokens);
    println!("{:#?}", nodes);
}




/*

| Left | Right | Result |
| ---- | ----- | ------ |
| NumberOffset | NumberOffset | NumberOffset |
| NumberOffset | NumberScale | NumberOffset |

| NumberScale | NumberScale | NumberScale |
| NumberScale | NumberOffset | NumberScale |
| Number | NumberScale | NumberScale |
| Number | NumberOffset | NumberScale |

*/
//! Scan a FEA file, producing a sequence of tokens.
//!
//! This is the first step in our parsing process. The tokens produced here
//! have no semantic information; for instance we do not try to distinguish a
//! keyword from a glyph name. Instead we are just describing the most basic
//! structure of the document.

use crate::token::{Kind, Token};

const EOF: u8 = 0x0;

pub(crate) struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    after_backslash: bool,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Lexer {
            input,
            pos: 0,
            after_backslash: false,
        }
    }

    fn nth(&self, index: usize) -> u8 {
        self.input
            .as_bytes()
            .get(self.pos + index)
            .copied()
            .unwrap_or(EOF)
    }

    fn bump(&mut self) -> Option<u8> {
        let pos = self.pos;
        let next = self.input.as_bytes().get(pos).copied();
        self.pos += if next.is_some() { 1 } else { 0 };
        next
    }

    pub(crate) fn next_token(&mut self) -> Token {
        let start_pos = self.pos;
        let first = self.bump().unwrap_or(EOF);
        let kind = match first {
            EOF => Kind::Eof,
            byte if is_ascii_whitespace(byte) => self.whitespace(),
            b'#' => self.comment(),
            b'"' => self.string(),
            b'0' => self.number(true),
            b'1'..=b'9' => self.number(false),
            b';' => Kind::Semi,
            b',' => Kind::Comma,
            b'@' => self.glyph_class_name(),
            b'\\' => Kind::Backslash,
            //b'\\' => self.backslash(),
            b'-' => Kind::Hyphen,
            b'=' => Kind::Eq,
            b'{' => Kind::LBrace,
            b'}' => Kind::RBrace,
            b'[' => Kind::LSquare,
            b']' => Kind::RSquare,
            b'(' => Kind::LParen,
            b')' => Kind::RParen,
            b'<' => Kind::LAngle,
            b'>' => Kind::RAngle,
            b'\'' => Kind::SingleQuote,
            _ => self.ident(),
        };

        self.after_backslash = matches!(kind, Kind::Backslash);

        let len = self.pos - start_pos;
        Token { len, kind }
    }

    fn whitespace(&mut self) -> Kind {
        while is_ascii_whitespace(self.nth(0)) {
            self.bump();
        }
        Kind::Whitespace
    }

    fn comment(&mut self) -> Kind {
        while [b'\n', EOF].contains(&self.nth(0)) {
            self.bump();
        }
        Kind::Comment
    }

    fn string(&mut self) -> Kind {
        loop {
            match self.nth(0) {
                b'"' => {
                    self.bump();
                    break Kind::String;
                }
                EOF => break Kind::StringUnterminated,
                _ => {
                    self.bump();
                }
            }
        }
    }

    fn number(&mut self, leading_zero: bool) -> Kind {
        if leading_zero && [b'x', b'X'].contains(&self.nth(0)) {
            self.bump();
            if self.nth(0).is_ascii_hexdigit() {
                self.eat_hex_digits();
                Kind::NumberHex
            } else {
                Kind::NumberHexEmpty
            }
        } else {
            self.eat_decimal_digits();
            if self.nth(0) == b'.' {
                self.bump();
                self.eat_decimal_digits();
                Kind::NumberFloat
            } else {
                Kind::NumberDec
            }
        }
    }

    fn eat_hex_digits(&mut self) {
        while self.nth(0).is_ascii_hexdigit() {
            self.bump();
        }
    }

    fn eat_decimal_digits(&mut self) {
        while self.nth(0).is_ascii_digit() {
            self.bump();
        }
    }

    fn glyph_class_name(&mut self) -> Kind {
        self.eat_ident();
        Kind::NamedGlyphClass
    }

    fn eat_ident(&mut self) {
        loop {
            match self.nth(0) {
                EOF => break,
                b if is_ascii_whitespace(b) => break,
                b'-' => (),
                b if is_special(b) => break,
                _ => (),
            }
            self.bump();
        }
    }

    /// super dumb for now; we eat anything that isn't whitespace or special char.
    fn ident(&mut self) -> Kind {
        let start_pos = self.pos.saturating_sub(1);
        self.eat_ident();

        if self.after_backslash {
            return Kind::Ident;
        }

        let raw_token = &self.input.as_bytes()[start_pos..self.pos];
        Kind::from_keyword(raw_token).unwrap_or(Kind::Ident)
    }
}

//pub(crate) fn iter_tokens(text: &str) -> impl Iterator<Item = Token> + '_ {
//let mut cursor = Lexer::new(text);
//std::iter::from_fn(move || {
//let next = cursor.next_token();
//match next.kind {
//Kind::Eof => None,
//_ => Some(next),
//}
//})
//}

// [\ , ' - ; < = > @ \ ( ) [ ] { }]
fn is_special(byte: u8) -> bool {
    (39..=45).contains(&byte)
        || (59..=64).contains(&byte)
        || (91..=93).contains(&byte)
        || byte == 123
        || byte == 125
}

//pub(crate) fn tokenize(text: &str) -> Vec<Token> {
//iter_tokens(text).collect()
//}

fn is_ascii_whitespace(byte: u8) -> bool {
    byte == b' ' || (0x9..=0xD).contains(&byte)
}

#[cfg(test)]
pub(crate) fn debug_tokens(tokens: &[Token]) -> Vec<String> {
    let mut result = Vec::new();
    let mut pos = 0;
    for token in tokens {
        result.push(format!("{}..{} {}", pos, pos + token.len, token.kind));
        pos += token.len;
    }
    result
}

#[cfg(test)]
pub(crate) fn debug_tokens2(tokens: &[Token], src: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut pos = 0;
    for token in tokens {
        let text = if token.kind.has_contents() {
            format!("{}({})", token.kind, &src[pos..pos + token.len])
        } else {
            format!("{}", token.kind)
        };
        result.push(text);
        pos += token.len;
    }
    result
}

// microbenchmarks to do, one day. Who do you think will win??

//fn is_special_match(byte: u8) -> bool {
//match byte {
//b';' | b',' | b'@' | b'\\' | b'-' | b'=' | b'{' | b'}' | b'[' | b']' | b'(' | b')'
//| b'<' | b'>' | b'\'' => true,
//_ => false,
//}
//}

//fn is_special_ranges(byte: u8) -> bool {
//(39..=45).contains(&byte)
//|| (59..=64).contains(&byte)
//|| (91..=93).contains(&byte)
//|| byte == 123
//|| byte == 125
//}

//fn is_special_bsearch(byte: u8) -> bool {
//[39, 40, 41, 44, 45, 59, 60, 61, 62, 64, 91, 92, 93, 123, 125]
//.binary_search(&byte)
//.is_ok()
//}

//// could improve this by sorting by frequency
//fn is_special_linear_scan(byte: u8) -> bool {
//[39, 40, 41, 44, 45, 59, 60, 61, 62, 64, 91, 92, 93, 123, 125].contains(&byte)
//}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_hex() {
        let fea = "0x 0x11 0xzz";
        let tokens = tokenize(fea);
        let token_strs = debug_tokens(&tokens);
        assert_eq!(token_strs[0], "0..2 HEX EMPTY");
        assert_eq!(token_strs[1], "2..3 WS");
        assert_eq!(token_strs[2], "3..7 HEX");
        assert_eq!(token_strs[3], "7..8 WS");
        assert_eq!(token_strs[4], "8..10 HEX EMPTY");
        assert_eq!(token_strs[5], "10..12 ID");
    }

    #[test]
    fn languagesystem() {
        let fea = "languagesystem dflt cool;";
        let tokens = tokenize(fea);
        assert_eq!(tokens[0].len, 14);
        let token_strs = debug_tokens2(&tokens, fea);
        assert_eq!(token_strs[0], "LanguagesystemKw");
        assert_eq!(token_strs[1], "WS( )");
        assert_eq!(token_strs[2], "ID(dflt)");
        assert_eq!(token_strs[3], "WS( )");
        assert_eq!(token_strs[4], "ID(cool)");
        assert_eq!(token_strs[5], ";");
    }

    #[test]
    fn escaping_keywords() {
        let fea = "sub \\sub \\rsub";
        let tokens = tokenize(fea);
        let token_strs = debug_tokens2(&tokens, fea);
        assert_eq!(token_strs[0], "SubKw");
        assert_eq!(token_strs[1], "WS( )");
        assert_eq!(token_strs[2], "\\");
        assert_eq!(token_strs[3], "ID(sub)");
        assert_eq!(token_strs[4], "WS( )");
        assert_eq!(token_strs[5], "\\");
        assert_eq!(token_strs[6], "ID(rsub)");
    }

    #[test]
    fn cid_versus_ident() {
        let fea = "@hi =[\\1-\\2 a - b];";
        let tokens = tokenize(fea);
        let token_strs = debug_tokens2(&tokens, fea);
        assert_eq!(token_strs[0], "@GlyphClass(@hi)");
        assert_eq!(token_strs[1], "WS( )");
        assert_eq!(token_strs[2], "=");
        assert_eq!(token_strs[3], "[");
        assert_eq!(token_strs[4], "\\");
        assert_eq!(token_strs[5], "DEC(1)");
        assert_eq!(token_strs[6], "-");
        assert_eq!(token_strs[7], "\\");
        assert_eq!(token_strs[8], "DEC(2)");
        assert_eq!(token_strs[9], "WS( )");
        assert_eq!(token_strs[10], "ID(a)");
        assert_eq!(token_strs[11], "WS( )");
        assert_eq!(token_strs[12], "-");
        assert_eq!(token_strs[13], "WS( )");
        assert_eq!(token_strs[14], "ID(b)");
        assert_eq!(token_strs[15], "]");
        assert_eq!(token_strs[16], ";");
    }
}

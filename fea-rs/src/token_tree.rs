use std::{cell::Cell, ops::Range, sync::Arc};

use smol_str::SmolStr;

use crate::{
    parse::{SyntaxError, TokenComparable, TreeSink},
    GlyphMap, Kind, TokenSet,
};

use self::cursor::Cursor;

mod cursor;
mod edit;
mod stack;

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct Node {
    pub kind: Kind,
    // start of this node relative to start of parent node.
    // we can use this to more efficiently move to a given offset
    // TODO: remove if unused
    rel_pos: u32,

    // NOTE: the absolute position within the tree is not known when the node
    // is created; this is updated (and correct) only when the node has been
    // accessed via a `Cursor`.
    abs_pos: Cell<u32>,
    text_len: u32,
    // true if an error was encountered in this node. this is not recursive;
    // it is only true for the direct parent of an error span.
    pub error: bool,
    //NOTE: children should not be accessed directly, but only via a cursor.
    // this ensures that their positions are updated correctly.
    children: Arc<Vec<NodeOrToken>>,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct Token {
    pub kind: Kind,
    abs_pos: Cell<u32>,
    pub text: SmolStr,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeOrToken {
    Node(Node),
    Token(Token),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TreeBuilder {
    //TODO: reuse tokens
    //token_cache: HashMap<Arc<Token>>,
    // the kind of the parent, and the index in children of the first child.
    parents: Vec<(Kind, usize)>,
    children: Vec<NodeOrToken>,
}

pub struct AstSink<'a> {
    text: &'a str,
    text_pos: usize,
    builder: TreeBuilder,
    glyph_map: Option<&'a GlyphMap>,
    errors: Vec<SyntaxError>,
    cur_node_contains_error: bool,
}

impl TreeSink for AstSink<'_> {
    fn token(&mut self, kind: Kind, len: usize) {
        let token_text = &self.text[self.text_pos..self.text_pos + len];
        let to_add = self.validate_token(kind, token_text);
        self.builder.push_raw(to_add);
        self.text_pos += len;
    }

    fn start_node(&mut self, kind: Kind) {
        self.builder.start_node(kind);
    }

    fn finish_node(&mut self) {
        self.builder.finish_node(self.cur_node_contains_error);
        self.cur_node_contains_error = false;
    }

    fn error(&mut self, error: SyntaxError) {
        self.errors.push(error);
        self.cur_node_contains_error = true;
    }
}

impl<'a> AstSink<'a> {
    pub fn new(text: &'a str, glyph_map: Option<&'a GlyphMap>) -> Self {
        AstSink {
            text,
            text_pos: 0,
            builder: TreeBuilder::default(),
            glyph_map,
            errors: Vec::new(),
            cur_node_contains_error: false,
        }
    }

    pub fn finish(self) -> (Node, Vec<SyntaxError>) {
        let node = self.builder.finish();
        (node, self.errors)
    }

    /// called before adding a token.
    ///
    /// We can perform additional validation here. Currently it is mostly for
    /// disambiguating glyph names that might be ranges.
    fn validate_token(&mut self, kind: Kind, text: &str) -> NodeOrToken {
        if kind == Kind::GlyphNameOrRange {
            if let Some(map) = self.glyph_map {
                if map.contains(text) {
                    return Token::new(Kind::GlyphName, text.into()).into();
                }
                match try_split_range(text, map) {
                    Ok(node) => return node.into(),
                    Err(message) => {
                        let range = self.text_pos..self.text_pos + text.len();
                        self.error(SyntaxError { message, range });
                    }
                }
            }
        }
        Token::new(kind, text.into()).into()
    }
}

impl Node {
    fn new(kind: Kind, mut children: Vec<NodeOrToken>, error: bool) -> Self {
        let mut text_len = 0;
        for child in &mut children {
            if let NodeOrToken::Node(n) = child {
                n.rel_pos += text_len;
            }
            text_len += child.text_len() as u32;
        }

        Node {
            kind,
            text_len,
            rel_pos: 0,
            abs_pos: Cell::new(0),
            children: children.into(),
            error,
        }
    }

    pub fn cursor(&self) -> Cursor {
        Cursor::new(self)
    }

    pub fn iter_tokens(&self) -> impl Iterator<Item = &Token> {
        let mut cursor = Cursor::new(self);
        std::iter::from_fn(move || cursor.next_token())
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn text_len(&self) -> usize {
        self.text_len as usize
    }

    /// The range in the original source of this node.
    ///
    /// Only correct if this node is accessed via a cursor.
    pub fn range(&self) -> Range<usize> {
        let start = self.abs_pos.get() as usize;
        start..start + (self.text_len as usize)
    }

    #[doc(hidden)]
    pub fn debug_print_structure(&self, include_tokens: bool) {
        let mut cursor = self.cursor();
        while let Some(thing) = cursor.current() {
            match thing {
                NodeOrToken::Node(node) => {
                    let depth = cursor.depth();
                    eprintln!(
                        "{}{} ({}..{})",
                        &crate::util::SPACES[..depth * 2],
                        node.kind,
                        cursor.pos(),
                        cursor.pos() + node.text_len()
                    );
                }
                NodeOrToken::Token(t) if include_tokens => eprint!("{}", t.as_str()),
                _ => (),
            }
            cursor.advance();
        }
    }
}

impl TreeBuilder {
    pub(crate) fn start_node(&mut self, kind: Kind) {
        let len = self.children.len();
        self.parents.push((kind, len));
    }

    pub(crate) fn token(&mut self, kind: Kind, text: impl Into<SmolStr>) {
        let token = Token::new(kind, text.into());
        self.push_raw(token.into());
    }

    fn push_raw(&mut self, item: NodeOrToken) {
        self.children.push(item)
    }

    pub(crate) fn finish_node(&mut self, error: bool) {
        let (kind, first_child) = self.parents.pop().unwrap();
        let node = Node::new(kind, self.children.split_off(first_child), error);
        self.push_raw(node.into());
    }

    pub(crate) fn finish(mut self) -> Node {
        assert_eq!(self.children.len(), 1);
        self.children.pop().unwrap().into_node().unwrap()
    }
}

impl NodeOrToken {
    pub(crate) fn set_abs_pos(&self, pos: usize) {
        match self {
            NodeOrToken::Token(t) => t.abs_pos.set(pos as u32),
            NodeOrToken::Node(n) => n.abs_pos.set(pos as u32),
        }
    }

    pub fn is_token(&self) -> bool {
        matches!(self, NodeOrToken::Token(_))
    }

    pub fn token_text(&self) -> Option<&str> {
        self.as_token().map(Token::as_str)
    }

    pub fn kind(&self) -> Kind {
        match self {
            NodeOrToken::Node(n) => n.kind,
            NodeOrToken::Token(t) => t.kind,
        }
    }

    /// The range in the source text of this node or token.
    ///
    /// Note: this is only accurate if the token was accessed via a cursor.
    pub fn range(&self) -> Range<usize> {
        match self {
            NodeOrToken::Token(t) => t.range(),
            NodeOrToken::Node(n) => n.range(),
        }
    }

    pub fn matches(&self, predicate: TokenSet) -> bool {
        predicate.matches(self.kind())
    }

    pub fn text_len(&self) -> usize {
        match self {
            NodeOrToken::Node(n) => n.text_len as usize,
            NodeOrToken::Token(t) => t.text.len(),
        }
    }

    pub fn into_node(self) -> Option<Node> {
        match self {
            NodeOrToken::Node(node) => Some(node),
            NodeOrToken::Token(_) => None,
        }
    }

    pub fn as_node(&self) -> Option<&Node> {
        match self {
            NodeOrToken::Node(node) => Some(node),
            NodeOrToken::Token(_) => None,
        }
    }

    pub fn as_token(&self) -> Option<&Token> {
        match self {
            NodeOrToken::Node(_) => None,
            NodeOrToken::Token(token) => Some(token),
        }
    }
}

impl From<Node> for NodeOrToken {
    fn from(src: Node) -> NodeOrToken {
        NodeOrToken::Node(src)
    }
}

impl From<Token> for NodeOrToken {
    fn from(src: Token) -> NodeOrToken {
        NodeOrToken::Token(src)
    }
}

impl Token {
    fn new(kind: Kind, text: SmolStr) -> Self {
        Token {
            kind,
            text,
            abs_pos: Cell::new(0),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn range(&self) -> Range<usize> {
        self.abs_pos.get() as usize..self.abs_pos.get() as usize + self.text.len()
    }
}

/// try to split a glyph containing hyphens into a glyph range.
fn try_split_range(text: &str, glyph_map: &GlyphMap) -> Result<Node, String> {
    let mut solution = None;

    // we try all possible split points
    for idx in text
        .bytes()
        .enumerate()
        .filter_map(|(idx, b)| (b == b'-').then(|| idx))
    {
        let (head, tail) = text.split_at(idx);
        if glyph_map.contains(head) && glyph_map.contains(tail.trim_start_matches('-')) {
            if let Some(prev_idx) = solution.replace(idx) {
                let (head1, tail1) = text.split_at(prev_idx);
                let (head2, tail2) = text.split_at(idx);
                let message = format!("the name '{}' contains multiple possible glyph ranges ({} to {} and {} to {}). Please insert spaces around the '-' to clarify your intent.", text, head1, tail1.trim_end_matches('-'), head2, tail2.trim_end_matches('-'));
                return Err(message);
            }
        }
    }

    // if we have a solution, generate a new node
    solution
        .map(|idx| {
            let mut builder = TreeBuilder::default();
            builder.start_node(Kind::GlyphRange);
            let (head, tail) = text.split_at(idx);
            builder.token(Kind::GlyphName, head);
            builder.token(Kind::Hyphen, "-");
            builder.token(Kind::GlyphName, tail.trim_start_matches('-'));
            builder.finish_node(false);
            builder.finish()
        })
        .ok_or_else(|| {
            format!(
                "'{}' is neither a known glyph or a range of known glyphs",
                text
            )
        })
}

#[cfg(test)]
mod tests {
    use crate::Parser;

    use super::*;
    static SAMPLE_FEA: &str = include_str!("../test-data/mini.fea");

    #[test]
    fn token_iter() {
        let mut sink = AstSink::new(SAMPLE_FEA, None);
        let mut parser = Parser::new(SAMPLE_FEA, &mut sink);
        crate::root(&mut parser);
        let (root, _errs) = sink.finish();
        let reconstruct = root.iter_tokens().map(Token::as_str).collect::<String>();

        crate::assert_eq_str!(SAMPLE_FEA, reconstruct);
    }
}

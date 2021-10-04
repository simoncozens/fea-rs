use std::{
    convert::TryFrom,
    fmt::{Display, Formatter},
    num::TryFromIntError,
    rc::Rc,
};

use smol_str::SmolStr;

mod glyph_map;
mod metrics;
mod rules;
mod tag;

pub use glyph_map::GlyphMap;
pub use metrics::{Anchor, ValueRecord};
pub use rules::{gpos, gsub};
pub use tag::{InvalidTag, Tag};

pub type GlyphName = SmolStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphId(u16);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphClass(Rc<[GlyphId]>);

#[derive(Debug, Clone)]
pub enum GlyphOrClass {
    Glyph(GlyphId),
    Class(GlyphClass),
}

// the general case; different uses have different constraints, which
// we will not bother to have specific types for
pub struct GlyphSequence(Rc<[GlyphOrClass]>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GlyphIdent {
    Name(GlyphName),
    Cid(u32),
}

impl GlyphId {
    pub const NOTDEF: GlyphId = GlyphId(0);
}

impl From<GlyphOrClass> for GlyphClass {
    fn from(src: GlyphOrClass) -> GlyphClass {
        match src {
            GlyphOrClass::Class(class) => class,
            GlyphOrClass::Glyph(id) => id.into(),
        }
    }
}

impl std::iter::FromIterator<GlyphId> for GlyphClass {
    fn from_iter<T: IntoIterator<Item = GlyphId>>(iter: T) -> Self {
        GlyphClass(iter.into_iter().collect())
    }
}

impl std::iter::FromIterator<GlyphOrClass> for GlyphSequence {
    fn from_iter<T: IntoIterator<Item = GlyphOrClass>>(iter: T) -> Self {
        GlyphSequence(iter.into_iter().collect())
    }
}

impl<T: Into<GlyphName>> From<T> for GlyphIdent {
    fn from(src: T) -> Self {
        GlyphIdent::Name(src.into())
    }
}

impl From<u16> for GlyphId {
    fn from(src: u16) -> GlyphId {
        GlyphId(src)
    }
}

impl TryFrom<usize> for GlyphId {
    type Error = TryFromIntError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        u16::try_from(value).map(GlyphId)
    }
}

impl Display for GlyphIdent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            GlyphIdent::Name(name) => write!(f, "{}", name),
            GlyphIdent::Cid(cid) => write!(f, "Cid({})", cid),
        }
    }
}

impl GlyphClass {
    pub fn items(&self) -> &[GlyphId] {
        &self.0
    }

    //pub fn len(&self) -> usize {
    //self.0.len()
    //}
}

impl From<Vec<GlyphId>> for GlyphClass {
    fn from(src: Vec<GlyphId>) -> GlyphClass {
        GlyphClass(src.into())
    }
}

impl From<GlyphId> for GlyphClass {
    fn from(src: GlyphId) -> GlyphClass {
        let slice: &[_] = &[src];
        GlyphClass(slice.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LanguageSystem {
    pub script: Tag,
    pub language: Tag,
}
//! TODO doc

use derive::Parseable;
use super::AMLParseable;
use super::Error;

/// TODO doc
#[derive(Parseable)]
pub struct DefAlias {
	// TODO
}

/// TODO doc
#[derive(Parseable)]
pub struct DefName {
	// TODO
}

/// TODO doc
#[derive(Parseable)]
pub struct DefScope {
	// TODO
}

/// TODO doc
#[derive(Parseable)]
pub enum NameSpaceModifierObj {
	DefAlias(DefAlias),
	DefName(DefAlias),
	DefScope(DefAlias),
}

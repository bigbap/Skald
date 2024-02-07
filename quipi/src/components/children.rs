use crate::{Component, VersionedIndex};

#[derive(Component, Debug, PartialEq)]
pub struct CChildren {
    pub list: Vec<VersionedIndex>
}

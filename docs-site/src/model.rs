//! Site model built from the docs tree.

/// One page in a section (for the sidebar). `output_rel` is the root-relative
/// output path (e.g. "architecture/api.html").
#[derive(Debug, Clone)]
pub struct PageRef {
    pub title: String,
    pub order: i64,
    pub output_rel: String,
}

/// A sidebar section (one folder).
#[derive(Debug, Clone)]
pub struct Section {
    pub title: String,
    pub order: i64,
    pub index_rel: String, // "architecture/index.html"
    pub pages: Vec<PageRef>,
}

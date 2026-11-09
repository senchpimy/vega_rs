//! Port of `src/compile/data/index.ts`.

use indexmap::IndexMap;
use serde_json::Value;

use crate::compile::split::Split;

use super::dataflow::NodeRef;

/// A `Parse` map: field name -> parse type.
pub type Parse = IndexMap<String, Value>;

/// Port of `AncestorParse`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AncestorParse {
    pub explicit: indexmap::IndexMap<String, Value>,
    pub implicit: indexmap::IndexMap<String, Value>,
    pub parse_nothing: bool,
}

impl AncestorParse {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_explicit(
        explicit: IndexMap<String, Value>,
        implicit: IndexMap<String, Value>,
        parse_nothing: bool,
    ) -> Self {
        Self {
            explicit,
            implicit,
            parse_nothing,
        }
    }

    fn to_split(&self) -> Split {
        Split {
            explicit: self.explicit.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            implicit: self.implicit.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }

    pub fn clone_ancestor(&self) -> Self {
        self.clone()
    }

    pub fn combine(&self) -> IndexMap<String, Value> {
        let split = self.to_split();
        split.combine().into_iter().collect()
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.to_split().get(key)
    }

    pub fn get_with_explicit(&self, key: &str) -> crate::compile::split::Explicit {
        self.to_split().get_with_explicit(key)
    }

    pub fn set(&mut self, key: &str, value: Value, explicit: bool) {
        if explicit {
            self.implicit.shift_remove(key);
            self.explicit.insert(key.to_string(), value);
        } else {
            self.explicit.shift_remove(key);
            self.implicit.insert(key.to_string(), value);
        }
    }

    pub fn set_with_explicit(&mut self, key: &str, value: crate::compile::split::Explicit) {
        if let Some(v) = value.value {
            self.set(key, v, value.explicit);
        }
    }

    pub fn copy_all(&mut self, other: &Split) {
        for (k, v) in &other.explicit {
            self.explicit.insert(k.clone(), v.clone());
        }
        for (k, v) in &other.implicit {
            self.implicit.insert(k.clone(), v.clone());
        }
    }
}

/// Port of `DataComponent`.
#[derive(Clone, Default)]
pub struct DataComponent {
    /// A list of unique sources.
    pub sources: Vec<NodeRef>,
    /// Registry of output nodes.
    pub output_nodes: IndexMap<String, NodeRef>,
    /// How often is an output node used.
    pub output_node_ref_counts: IndexMap<String, i64>,
    /// The output node before aggregation.
    pub raw: Option<NodeRef>,
    /// The output node for scale domain before filter invalid.
    pub pre_filter_invalid: Option<NodeRef>,
    /// The output node for scale domain after filter invalid.
    pub post_filter_invalid: Option<NodeRef>,
    /// The main output node.
    pub main: Option<NodeRef>,
    /// For facets, the reference to the root node.
    pub facet_root: Option<NodeRef>,
    /// True if the data for this model is faceted.
    pub is_faceted: bool,
    /// Parse properties passed down from ancestors.
    pub ancestor_parse: Option<AncestorParse>,
}

impl std::fmt::Debug for DataComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataComponent")
            .field("sources", &self.sources.len())
            .field("output_nodes", &self.output_nodes.keys().collect::<Vec<_>>())
            .field("main", &self.main.is_some())
            .field("facet_root", &self.facet_root.is_some())
            .field("is_faceted", &self.is_faceted)
            .finish()
    }
}

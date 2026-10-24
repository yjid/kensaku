pub mod ast;
pub mod lexer;
pub mod parser;
pub mod router;
pub mod scope_trie;

pub use ast::{ActiveFilterDto, FileKind, Filter, Mode, Query, RangeOp, Scope, Term, TermField};
pub use parser::parse_query;
pub use router::{route_query, QueryRoute};
pub use scope_trie::{DirectoryTrie, ScopeResolution, TrieMatchResult};

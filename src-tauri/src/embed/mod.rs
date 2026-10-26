pub mod model;
pub mod provider;
pub mod queue;
pub mod scanner;
pub mod smart;
pub mod table;
pub mod units;
pub mod wordpiece;

pub use model::{DEFAULT_MODEL_ID, EMBEDDING_DIM, ModelRegistry, REAL_MODEL_ID};
pub use provider::EmbeddingProviderKind;
pub use queue::EmbeddingQueue;
pub use scanner::{ExactScanner, SemanticHit};
pub use table::StaticTable;
pub use units::{Unit, UnitStore, chunk_text};

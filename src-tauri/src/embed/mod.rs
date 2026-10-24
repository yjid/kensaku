pub mod model;
pub mod queue;
pub mod scanner;
pub mod units;

pub use model::{ModelRegistry, DEFAULT_MODEL_ID, EMBEDDING_DIM};
pub use queue::EmbeddingQueue;
pub use scanner::{ExactScanner, SemanticHit};
pub use units::{chunk_text, Unit, UnitStore};

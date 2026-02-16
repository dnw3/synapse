mod runnable;
pub use runnable::{BoxRunnable, Runnable, RunnableOutputStream};

mod passthrough;
pub use passthrough::RunnablePassthrough;

mod lambda;
pub use lambda::RunnableLambda;

mod sequence;
pub use sequence::RunnableSequence;

mod parallel;
pub use parallel::RunnableParallel;

mod branch;
pub use branch::RunnableBranch;

mod fallback;
pub use fallback::RunnableWithFallbacks;

/// Backward-compatible alias for `RunnablePassthrough`.
pub type IdentityRunnable = RunnablePassthrough;

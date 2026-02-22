use async_trait::async_trait;
use bson::{doc, DateTime as BsonDateTime};
use futures::TryStreamExt;
use mongodb::{Collection, Database, IndexModel};
use synaptic_core::SynapticError;
use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer};

/// MongoDB-backed graph checkpointer.
///
/// Stores graph state checkpoints in a MongoDB collection, suitable for
/// distributed deployments where multiple processes share checkpointed state.
///
/// # Example
///
/// ```rust,no_run
/// use synaptic_mongodb::MongoCheckpointer;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
/// let db = client.database("myapp");
/// let checkpointer = MongoCheckpointer::new(&db, "graph_checkpoints").await?;
/// # Ok(())
/// # }
/// ```
pub struct MongoCheckpointer {
    collection: Collection<bson::Document>,
}

impl MongoCheckpointer {
    /// Create a new `MongoCheckpointer` backed by the given MongoDB database and collection.
    ///
    /// Creates a compound index on `(thread_id, checkpoint_id)` and a secondary
    /// index on `(thread_id, seq)` for efficient ordered retrieval.
    pub async fn new(db: &Database, collection_name: &str) -> Result<Self, SynapticError> {
        let collection: Collection<bson::Document> = db.collection(collection_name);

        // Unique index on (thread_id, checkpoint_id) — deduplicates puts
        let unique_idx = IndexModel::builder()
            .keys(doc! { "thread_id": 1, "checkpoint_id": 1 })
            .options(
                mongodb::options::IndexOptions::builder()
                    .unique(true)
                    .build(),
            )
            .build();

        // Index on (thread_id, seq) for ordered listing and latest retrieval
        let seq_idx = IndexModel::builder()
            .keys(doc! { "thread_id": 1, "seq": 1 })
            .build();

        collection
            .create_index(unique_idx)
            .await
            .map_err(|e| SynapticError::Store(format!("MongoDB create unique index: {e}")))?;

        collection
            .create_index(seq_idx)
            .await
            .map_err(|e| SynapticError::Store(format!("MongoDB create seq index: {e}")))?;

        Ok(Self { collection })
    }
}

#[async_trait]
impl Checkpointer for MongoCheckpointer {
    async fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: &Checkpoint,
    ) -> Result<(), SynapticError> {
        // Serialize the checkpoint to JSON string for storage
        let state_json = serde_json::to_string(checkpoint)
            .map_err(|e| SynapticError::Store(format!("Serialize: {e}")))?;

        // Determine next seq number for this thread
        let count = self
            .collection
            .count_documents(doc! { "thread_id": &config.thread_id })
            .await
            .map_err(|e| SynapticError::Store(format!("MongoDB count: {e}")))?;

        let document = doc! {
            "thread_id": &config.thread_id,
            "checkpoint_id": &checkpoint.id,
            "seq": count as i64,
            "state": &state_json,
            "created_at": BsonDateTime::now(),
        };

        // Use upsert to be idempotent — same (thread_id, checkpoint_id) replaces
        self.collection
            .update_one(
                doc! {
                    "thread_id": &config.thread_id,
                    "checkpoint_id": &checkpoint.id
                },
                doc! { "$setOnInsert": document },
            )
            .with_options(
                mongodb::options::UpdateOptions::builder()
                    .upsert(true)
                    .build(),
            )
            .await
            .map_err(|e| SynapticError::Store(format!("MongoDB upsert: {e}")))?;

        Ok(())
    }

    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapticError> {
        let filter = if let Some(ref id) = config.checkpoint_id {
            doc! { "thread_id": &config.thread_id, "checkpoint_id": id }
        } else {
            doc! { "thread_id": &config.thread_id }
        };

        let opts = mongodb::options::FindOneOptions::builder()
            .sort(doc! { "seq": -1 })
            .build();

        let result = self
            .collection
            .find_one(filter)
            .with_options(opts)
            .await
            .map_err(|e| SynapticError::Store(format!("MongoDB find_one: {e}")))?;

        match result {
            None => Ok(None),
            Some(doc) => {
                let state_str = doc
                    .get_str("state")
                    .map_err(|e| SynapticError::Store(format!("MongoDB get state field: {e}")))?;
                let cp: Checkpoint = serde_json::from_str(state_str)
                    .map_err(|e| SynapticError::Store(format!("Deserialize: {e}")))?;
                Ok(Some(cp))
            }
        }
    }

    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapticError> {
        let filter = doc! { "thread_id": &config.thread_id };
        let opts = mongodb::options::FindOptions::builder()
            .sort(doc! { "seq": 1 })
            .build();

        let mut cursor = self
            .collection
            .find(filter)
            .with_options(opts)
            .await
            .map_err(|e| SynapticError::Store(format!("MongoDB find: {e}")))?;

        let mut checkpoints = Vec::new();
        while let Some(doc) = cursor
            .try_next()
            .await
            .map_err(|e| SynapticError::Store(format!("MongoDB cursor: {e}")))?
        {
            let state_str = doc
                .get_str("state")
                .map_err(|e| SynapticError::Store(format!("MongoDB get state field: {e}")))?;
            let cp: Checkpoint = serde_json::from_str(state_str)
                .map_err(|e| SynapticError::Store(format!("Deserialize: {e}")))?;
            checkpoints.push(cp);
        }

        Ok(checkpoints)
    }
}

use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use synapse_loaders::{DirectoryLoader, Loader};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn tempdir() -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("synapse-loader-test-{}-{}", std::process::id(), n));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn loads_files_from_directory() {
    let dir = tempdir();
    fs::write(dir.join("a.txt"), "content a").unwrap();
    fs::write(dir.join("b.txt"), "content b").unwrap();

    let loader = DirectoryLoader::new(&dir);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 2);
    // Files are sorted
    assert_eq!(docs[0].content, "content a");
    assert_eq!(docs[1].content, "content b");
}

#[tokio::test]
async fn filters_by_glob_pattern() {
    let dir = tempdir();
    fs::write(dir.join("doc.txt"), "text file").unwrap();
    fs::write(dir.join("data.csv"), "csv file").unwrap();

    let loader = DirectoryLoader::new(&dir).with_glob("*.txt");
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].content, "text file");
}

#[tokio::test]
async fn recursive_loading() {
    let dir = tempdir();
    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.join("top.txt"), "top").unwrap();
    fs::write(sub.join("nested.txt"), "nested").unwrap();

    let loader = DirectoryLoader::new(&dir)
        .with_recursive(true)
        .with_glob("*.txt");
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 2);
}

#[tokio::test]
async fn adds_source_metadata() {
    let dir = tempdir();
    fs::write(dir.join("test.txt"), "content").unwrap();

    let loader = DirectoryLoader::new(&dir);
    let docs = loader.load().await.unwrap();

    assert!(docs[0].metadata.contains_key("source"));
}

use synaptic_loaders::{CsvLoader, Loader};

#[tokio::test]
async fn loads_csv_with_content_column() {
    let csv = "id,text,category\n1,Hello World,greeting\n2,Goodbye,farewell";

    let loader = CsvLoader::new(csv)
        .with_id_column("id")
        .with_content_column("text");
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].id, "1");
    assert_eq!(docs[0].content, "Hello World");
    assert_eq!(docs[1].id, "2");
    assert_eq!(docs[1].content, "Goodbye");
}

#[tokio::test]
async fn loads_csv_concatenates_all_columns() {
    let csv = "name,age\nAlice,30\nBob,25";

    let loader = CsvLoader::new(csv);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].id, "row-0");
    assert_eq!(docs[0].content, "Alice 30");
    assert!(docs[0].metadata.contains_key("name"));
}

#[tokio::test]
async fn csv_stores_metadata() {
    let csv = "title,body\nDoc1,Content1";

    let loader = CsvLoader::new(csv).with_content_column("body");
    let docs = loader.load().await.unwrap();

    assert_eq!(docs[0].metadata.get("title").unwrap(), "Doc1");
    assert_eq!(docs[0].metadata.get("body").unwrap(), "Content1");
}

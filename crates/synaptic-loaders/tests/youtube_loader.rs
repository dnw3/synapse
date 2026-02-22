use synaptic_loaders::YoutubeLoader;

#[test]
fn test_youtube_loader_new() {
    let loader = YoutubeLoader::new(vec!["dQw4w9WgXcQ".to_string()]);
    let _ = loader;
}

#[test]
fn test_youtube_loader_language() {
    let loader = YoutubeLoader::new(vec![]).with_language("zh");
    let _ = loader;
}

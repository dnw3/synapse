use synaptic_splitters::{Language, RecursiveCharacterTextSplitter, TextSplitter};

#[test]
fn python_splits_on_class_and_def() {
    let code = r#"
class MyClass:
    def __init__(self):
        self.x = 1

    def method(self):
        return self.x

def standalone_function():
    return 42

class AnotherClass:
    def another_method(self):
        pass
"#;

    let splitter = RecursiveCharacterTextSplitter::from_language(Language::Python, 80, 0);
    let chunks = splitter.split_text(code);

    assert!(
        chunks.len() >= 2,
        "expected at least 2 chunks, got {}",
        chunks.len()
    );
    // Each chunk should be within size limit
    for chunk in &chunks {
        assert!(
            chunk.len() <= 80,
            "chunk too long: {} chars: {:?}",
            chunk.len(),
            chunk
        );
    }
}

#[test]
fn rust_splits_on_fn_and_struct() {
    let code = r#"
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

fn main() {
    let p1 = Point::new(0.0, 0.0);
    let p2 = Point::new(3.0, 4.0);
    println!("{}", p1.distance(&p2));
}
"#;

    let splitter = RecursiveCharacterTextSplitter::from_language(Language::Rust, 100, 0);
    let chunks = splitter.split_text(code);

    assert!(
        chunks.len() >= 2,
        "expected at least 2 chunks, got {}",
        chunks.len()
    );
    for chunk in &chunks {
        assert!(
            chunk.len() <= 100,
            "chunk too long: {} chars: {:?}",
            chunk.len(),
            chunk
        );
    }
}

#[test]
fn language_separators_all_end_with_empty() {
    let languages = [
        Language::Python,
        Language::JavaScript,
        Language::TypeScript,
        Language::Rust,
        Language::Go,
        Language::Java,
        Language::Cpp,
        Language::Ruby,
        Language::Markdown,
        Language::Latex,
        Language::Html,
    ];

    for lang in &languages {
        let seps = lang.separators();
        assert!(!seps.is_empty(), "language {:?} has no separators", lang);
        assert_eq!(
            seps.last().unwrap(),
            "",
            "language {:?} should end with empty separator",
            lang
        );
    }
}

#[test]
fn from_language_with_overlap() {
    let splitter = RecursiveCharacterTextSplitter::from_language(Language::Python, 50, 10);
    let code = "def foo():\n    return 1\n\ndef bar():\n    return 2\n\ndef baz():\n    return 3\n";
    let chunks = splitter.split_text(code);

    assert!(chunks.len() >= 2);
}

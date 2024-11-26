#![allow(unused)]

use kalosm::language::{kalosm_sample, Parse, Schema};
use pretty_assertions::assert_eq;

#[derive(Parse, Schema, Clone)]
#[parse(tag = "ty", content = "contents")]
enum NamedEnum {
    #[parse(rename = "person")]
    Person {
        name: String,
        #[parse(range = 0..=100)]
        age: u32,
    },
    #[parse(rename = "animal")]
    Animal {
        #[parse(len = 1..=20)]
        name: String,
        #[parse(pattern = "cat|dog|bird")]
        species: String,
    },
}

#[cfg(any(feature = "metal", feature = "cuda"))]
#[tokio::test]
async fn named_enum() {
    use kalosm::language::*;

    let model = Llama::builder()
        .with_source(LlamaSource::tiny_llama_1_1b_chat())
        .build()
        .await
        .unwrap();

    let task = Task::builder("You generate json")
        .with_constraints(NamedEnum::new_parser())
        .build();

    let output = task
        .run("What is the capital of France?", &model)
        .all_text()
        .await;
    println!("{output}");

    assert!(output.contains("\"ty\":"));
    assert!(output.contains("\"contents\":"));
    assert!(output.contains("\"ty\": \"person\"") || output.contains("\"ty\": \"animal\""));
    assert!(output.contains("\"name\":"));
    assert!(output.contains("\"age\":") || output.contains("\"species\":"));
}

#[derive(Parse, Schema, Clone)]
enum MixedEnum {
    Person {
        #[parse(rename = "person")]
        name: String,
        age: u32,
    },
    Animal,
    Turtle(String),
}

#[test]
fn mixed_enum_schema() {
    let schema = MixedEnum::schema();
    let json = serde_json::from_str::<serde_json::Value>(&schema.to_string()).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "oneOf": [
                {
                    "type": "object",
                    "properties": {
                        "type": { "const": "Person" },
                        "data": {
                            "type": "object",
                            "properties": {
                                "person": {
                                    "type": "string"
                                },
                                "age": { "type": "integer" }
                            },
                            "required": ["person", "age"],
                            "additionalProperties": false
                        }
                    },
                    "required": ["type", "data"],
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "properties": {
                        "type": { "const": "Animal" }
                    },
                    "required": ["type"],
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "properties": {
                        "type": { "const": "Turtle" },
                        "data": { "type": "string" }
                    },
                    "required": ["type", "data"],
                    "additionalProperties": false
                }
            ]
        })
    );
}

#[cfg(any(feature = "metal", feature = "cuda"))]
#[tokio::test]
async fn mixed_enum() {
    use kalosm::language::*;

    let model = Llama::builder()
        .with_source(LlamaSource::tiny_llama_1_1b_chat())
        .build()
        .await
        .unwrap();

    let task = Task::builder("You generate json")
        .with_constraints(MixedEnum::new_parser())
        .build();

    let output = task
        .run("What is the capital of France?", &model)
        .all_text()
        .await;
    println!("{output}");
}

#[derive(Parse, Schema, Clone)]
enum UnitEnum {
    /// The first variant
    First,
    /// The other variant
    #[parse(rename = "second")]
    Second,
}

#[test]
fn unit_enum_schema() {
    let schema = UnitEnum::schema();
    let json = serde_json::from_str::<serde_json::Value>(&schema.to_string()).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "enum": ["First", "second"]
        })
    )
}

#[cfg(any(feature = "metal", feature = "cuda"))]
#[tokio::test]
async fn unit_enum() {
    use kalosm::language::*;

    let model = Llama::builder()
        .with_source(LlamaSource::tiny_llama_1_1b_chat())
        .build()
        .await
        .unwrap();

    let task = Task::builder("You generate json")
        .with_constraints(UnitEnum::new_parser())
        .build();

    let output = task
        .run("What is the capital of France?", &model)
        .all_text()
        .await;
    println!("{output}");
}

#[derive(Parse, Schema, Clone)]
enum TupleEnum {
    First(String),
    Second(String),
}

#[cfg(any(feature = "metal", feature = "cuda"))]
#[tokio::test]
async fn tuple_enum() {
    use kalosm::language::*;
    let model = Llama::builder()
        .with_source(LlamaSource::tiny_llama_1_1b_chat())
        .build()
        .await
        .unwrap();

    let task = Task::builder("You generate json")
        .with_constraints(TupleEnum::new_parser())
        .build();

    let output = task
        .run("What is the capital of France?", &model)
        .all_text()
        .await;
    println!("{output}");

    assert!(output.contains("\"type\": \"First\"") || output.contains("\"type\": \"Second\""));
}

#[test]
fn unit_enum_parses() {
    #[derive(Parse, Schema, Debug, Clone, PartialEq)]
    enum Color {
        Red,
        Blue,
        Green,
    }

    {
        let parser = Color::new_parser();
        use kalosm::language::{CreateParserState, Parser};
        let state = parser.create_parser_state();
        let color = parser.parse(&state, b"\"Red\" ").unwrap().unwrap_finished();
        assert_eq!(color, Color::Red);
    }
}

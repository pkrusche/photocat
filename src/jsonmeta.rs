use serde_json::Value;

pub fn merge(a: &mut Value, b: Value) {
    match (a, b) {
        (a @ &mut Value::Object(_), Value::Object(b)) => {
            let a = a.as_object_mut().unwrap();
            for (k, v) in b {
                merge(a.entry(k).or_insert(Value::Null), v);
            }
        }
        (a @ &mut Value::Array(_), b @ Value::Array(_)) => {
            let a = a.as_array_mut().unwrap();
            for item in b.as_array().unwrap() {
                a.push(item.clone());
            }
        }
        (a, b) => *a = b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge() {
        let mut a = json!({
            "string": "This is a string",
            "dict" : {
                "a" : "A",
                "b" : "B"
            },
            "array":[ "I", "J" ]
        });

        let b = json!({
            "string": "This is another string",
            "dict" : {
                "a" : "A_2"
            },
            "array":[ "K" ]
        });

        merge(&mut a, b);

        let expected = json!({
            "string": "This is another string",
            "dict" : {
                "a" : "A_2",
                "b" : "B"
            },
            "array":[ "I", "J", "K" ]
        });

        assert_eq!(a, expected);
    }

    #[test]
    fn test_merge_with_empty_object() {
        let mut a = json!({
            "string": "This is a string",
            "dict" : {
                "a" : "A",
                "b" : "B"
            },
            "array":[ "I", "J" ]
        });

        let b = json!({});

        merge(&mut a, b);

        let expected = json!({
            "string": "This is a string",
            "dict" : {
                "a" : "A",
                "b" : "B"
            },
            "array":[ "I", "J" ]
        });

        assert_eq!(a, expected);
    }

    #[test]
    fn test_merge_with_empty_array() {
        let mut a = json!({
            "string": "This is a string",
            "dict" : {
                "a" : "A",
                "b" : "B"
            },
            "array":[ "I", "J" ]
        });

        let b = json!({
            "array": []
        });

        merge(&mut a, b);

        let expected = json!({
            "string": "This is a string",
            "dict" : {
                "a" : "A",
                "b" : "B"
            },
            "array":[ "I", "J" ]
        });

        assert_eq!(a, expected);
    }
}

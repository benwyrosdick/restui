use jaq_interpret::{Ctx, FilterT, ParseCtx, RcIter, Val};

/// Apply a jq-style filter to JSON input
pub fn apply_jq_filter(json: &str, query: &str) -> Result<String, String> {
    // Parse the JSON input
    let input: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("Invalid JSON: {}", e))?;

    // Parse the filter
    let (filter, errs) = jaq_parse::parse(query, jaq_parse::main());

    if !errs.is_empty() {
        return Err(format!(
            "Parse error: {}",
            errs.into_iter()
                .map(|e| format!("{:?}", e))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let filter = filter.ok_or_else(|| "Failed to parse filter".to_string())?;

    // Create parsing context - use empty defs for basic operations
    // Basic operations like ., .field, .[0], .[] are built-in
    let mut ctx = ParseCtx::new(Vec::new());

    // Compile the filter
    let compiled = ctx.compile(filter);
    if !ctx.errs.is_empty() {
        return Err(format!("Compile error: {} errors", ctx.errs.len()));
    }

    // Execute the filter
    let inputs = RcIter::new(std::iter::empty());
    let val = Val::from(input);

    let results: Vec<Result<Val, _>> = compiled.run((Ctx::new([], &inputs), val)).collect();

    // Format output
    let mut output_parts = Vec::new();
    for result in results {
        match result {
            Ok(v) => {
                let json_val: serde_json::Value = v.into();
                let pretty = serde_json::to_string_pretty(&json_val)
                    .unwrap_or_else(|_| json_val.to_string());
                output_parts.push(pretty);
            }
            Err(_) => {
                return Err("Filter execution error".to_string());
            }
        }
    }

    if output_parts.is_empty() {
        Ok("null".to_string())
    } else {
        Ok(output_parts.join("\n---\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_filter() {
        let json = r#"{"name": "test"}"#;
        let result = apply_jq_filter(json, ".").unwrap();
        assert!(result.contains("name"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_field_access() {
        let json = r#"{"name": "test", "value": 42}"#;
        let result = apply_jq_filter(json, ".name").unwrap();
        assert!(result.contains("test"));
    }

    #[test]
    fn test_array_access() {
        let json = r#"[1, 2, 3]"#;
        let result = apply_jq_filter(json, ".[0]").unwrap();
        assert!(result.contains("1"));
    }

    #[test]
    fn test_invalid_json() {
        let result = apply_jq_filter("not json", ".");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_filter() {
        let json = r#"{"name": "test"}"#;
        let result = apply_jq_filter(json, ".invalid[");
        assert!(result.is_err());
    }
}

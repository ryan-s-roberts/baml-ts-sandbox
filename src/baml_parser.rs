//! BAML file parser

use crate::baml_execution::{BamlFunction, BamlParameter};
use crate::error::{BamlRtError, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Parse BAML files and extract function definitions
pub struct BamlParser;

impl BamlParser {
    /// Parse a BAML file and extract function definitions
    pub fn parse_file(file_path: &Path) -> Result<Vec<BamlFunction>> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| BamlRtError::Io(e))?;

        Self::parse_content(&content)
    }

    /// Parse BAML content and extract function definitions
    pub fn parse_content(content: &str) -> Result<Vec<BamlFunction>> {
        let mut functions = Vec::new();

        // Pattern to match BAML function definitions - simplified to handle our case
        let function_pattern = Regex::new(
            r"function\s+(\w+)\s*\(([^)]*)\)\s*->\s*(\w+)\s*\{([^}]+)\}"
        ).unwrap();

        for cap in function_pattern.captures_iter(content) {
            let name = cap.get(1).unwrap().as_str().to_string();
            let params_str = cap.get(2).unwrap().as_str();
            let _return_type = cap.get(3).unwrap().as_str();
            let body = cap.get(4).unwrap().as_str();

            // Parse parameters
            let parameters = Self::parse_parameters(params_str)?;

            // Extract client
            let client_pattern = Regex::new(r#"client\s+"([^"]+)""#).unwrap();
            let client = client_pattern
                .captures(body)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            // Extract prompt template - handle #"..."# format
            let prompt_pattern = Regex::new(r#"prompt\s+#"([^"]+)"#"#).unwrap();
            let mut prompt_template = String::new();
            if let Some(cap) = prompt_pattern.captures(body) {
                if let Some(m) = cap.get(1) {
                    prompt_template = m.as_str().trim().to_string();
                }
            }

            functions.push(BamlFunction {
                name,
                parameters,
                client,
                prompt_template,
            });
        }

        Ok(functions)
    }

    /// Parse function parameters from a parameter string
    fn parse_parameters(params_str: &str) -> Result<Vec<BamlParameter>> {
        let mut parameters = Vec::new();

        if params_str.trim().is_empty() {
            return Ok(parameters);
        }

        // Split by comma and parse each parameter
        for param_str in params_str.split(',') {
            let param_str = param_str.trim();
            if param_str.is_empty() {
                continue;
            }

            // Parse name:type pattern
            let parts: Vec<&str> = param_str.split(':').map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(BamlRtError::InvalidArgument(
                    String::from("Invalid parameter")
                ));
            }

            parameters.push(BamlParameter {
                name: parts[0].to_string(),
                param_type: parts[1].to_string(),
            });
        }

        Ok(parameters)
    }
}

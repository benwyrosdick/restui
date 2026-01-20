use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

fn interpolate_with_vars(vars: &HashMap<String, String>, input: &str) -> String {
    let re = Regex::new(r"\{\{(\w+)\}\}").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let var_name = &caps[1];
        vars.get(var_name)
            .cloned()
            .unwrap_or_else(|| format!("{{{{{}}}}}", var_name))
    })
    .into_owned()
}

/// An environment with variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub name: String,
    pub variables: HashMap<String, String>,
    /// Optional color for the header bar (e.g., "red", "green", "blue", "yellow", "magenta", "cyan")
    #[serde(default)]
    pub color: Option<String>,
}

impl Environment {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            variables: HashMap::new(),
            color: None,
        }
    }

    /// Set a variable value
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(key.into(), value.into());
    }

    /// Get a variable value
    pub fn get(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    /// Interpolate variables in a string using {{variable}} syntax
    pub fn interpolate(&self, input: &str) -> String {
        interpolate_with_vars(&self.variables, input)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new("default")
    }
}

/// Manager for multiple environments
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvironmentManager {
    #[serde(default)]
    pub shared: HashMap<String, String>,
    pub environments: Vec<Environment>,
    pub active_index: Option<usize>,
}

impl EnvironmentManager {
    pub fn new() -> Self {
        let mut manager = Self {
            shared: HashMap::new(),
            environments: Vec::new(),
            active_index: None,
        };
        // Create a default environment
        let mut default_env = Environment::new("default");
        default_env.set("base_url", "http://localhost:3000");
        manager.environments.push(default_env);
        manager.active_index = Some(0);
        manager
    }

    /// Load environments from a JSON file
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::new())
        }
    }

    /// Save environments to a JSON file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the currently active environment
    pub fn active(&self) -> Option<&Environment> {
        self.active_index.and_then(|i| self.environments.get(i))
    }

    /// Get the currently active environment mutably
    pub fn active_mut(&mut self) -> Option<&mut Environment> {
        self.active_index.and_then(|i| self.environments.get_mut(i))
    }

    /// Set the active environment by index
    pub fn set_active(&mut self, index: usize) {
        if index < self.environments.len() {
            self.active_index = Some(index);
        }
    }

    /// Add a new environment
    pub fn add(&mut self, env: Environment) {
        self.environments.push(env);
    }

    /// Interpolate a string using the active environment
    pub fn interpolate(&self, input: &str) -> String {
        match self.active() {
            Some(env) => {
                let mut combined = self.shared.clone();
                for (key, value) in &env.variables {
                    combined.insert(key.clone(), value.clone());
                }
                interpolate_with_vars(&combined, input)
            }
            None => interpolate_with_vars(&self.shared, input),
        }
    }

    /// Cycle to the next environment
    pub fn next(&mut self) {
        if !self.environments.is_empty() {
            let current = self.active_index.unwrap_or(0);
            self.active_index = Some((current + 1) % self.environments.len());
        }
    }

    /// Get the name of the active environment
    pub fn active_name(&self) -> &str {
        self.active().map(|e| e.name.as_str()).unwrap_or("none")
    }

    /// Get the color of the active environment (if set)
    pub fn active_color(&self) -> Option<&str> {
        self.active().and_then(|e| e.color.as_deref())
    }
}

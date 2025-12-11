//! Schema types for semantic validation
//!
//! These types represent the database schema that can be used for
//! schema-aware validation. The schema includes tables, columns,
//! and user-defined functions.

use serde::{Deserialize, Serialize};

/// Database schema for semantic validation
///
/// Contains definitions of tables, columns, and functions that
/// the KQL validator should be aware of when performing semantic
/// analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Schema {
    /// Database name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,

    /// Tables in the schema
    #[serde(default)]
    pub tables: Vec<Table>,

    /// User-defined functions
    #[serde(default)]
    pub functions: Vec<Function>,
}

impl Schema {
    /// Create a new empty schema
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a schema with a database name
    pub fn with_database(database: impl Into<String>) -> Self {
        Self {
            database: Some(database.into()),
            ..Self::default()
        }
    }

    /// Add a table to the schema
    pub fn add_table(&mut self, table: Table) -> &mut Self {
        self.tables.push(table);
        self
    }

    /// Add a function to the schema
    pub fn add_function(&mut self, function: Function) -> &mut Self {
        self.functions.push(function);
        self
    }

    /// Builder method to add a table
    pub fn table(mut self, table: Table) -> Self {
        self.tables.push(table);
        self
    }

    /// Builder method to add a function
    pub fn function(mut self, function: Function) -> Self {
        self.functions.push(function);
        self
    }

    /// Check if the schema is empty
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty() && self.functions.is_empty()
    }

    /// Get a table by name
    pub fn get_table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Get a function by name
    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
    }
}

/// Table definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    /// Table name
    pub name: String,

    /// Table columns
    #[serde(default)]
    pub columns: Vec<Column>,

    /// Optional table description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Table {
    /// Create a new table with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            columns: Vec::new(),
            description: None,
        }
    }

    /// Add a column to the table
    pub fn add_column(&mut self, column: Column) -> &mut Self {
        self.columns.push(column);
        self
    }

    /// Builder method to add a column
    pub fn column(mut self, column: Column) -> Self {
        self.columns.push(column);
        self
    }

    /// Builder method to add a column with name and type
    pub fn with_column(mut self, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        self.columns.push(Column::new(name, data_type));
        self
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Get a column by name
    pub fn get_column(&self, name: &str) -> Option<&Column> {
        self.columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }
}

/// Column definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    /// Column name
    pub name: String,

    /// KQL data type (string, long, datetime, dynamic, etc.)
    pub data_type: String,

    /// Optional column description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Column {
    /// Create a new column
    pub fn new(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            description: None,
        }
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Create a string column
    pub fn string(name: impl Into<String>) -> Self {
        Self::new(name, "string")
    }

    /// Create a long column
    pub fn long(name: impl Into<String>) -> Self {
        Self::new(name, "long")
    }

    /// Create a real column
    pub fn real(name: impl Into<String>) -> Self {
        Self::new(name, "real")
    }

    /// Create a bool column
    pub fn bool(name: impl Into<String>) -> Self {
        Self::new(name, "bool")
    }

    /// Create a datetime column
    pub fn datetime(name: impl Into<String>) -> Self {
        Self::new(name, "datetime")
    }

    /// Create a timespan column
    pub fn timespan(name: impl Into<String>) -> Self {
        Self::new(name, "timespan")
    }

    /// Create a guid column
    pub fn guid(name: impl Into<String>) -> Self {
        Self::new(name, "guid")
    }

    /// Create a dynamic column
    pub fn dynamic(name: impl Into<String>) -> Self {
        Self::new(name, "dynamic")
    }
}

/// User-defined function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Function name
    pub name: String,

    /// Parameter definitions
    #[serde(default)]
    pub parameters: Vec<Parameter>,

    /// Return type
    pub return_type: String,

    /// Optional function body (KQL expression)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Function {
    /// Create a new function
    pub fn new(name: impl Into<String>, return_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parameters: Vec::new(),
            return_type: return_type.into(),
            body: None,
            description: None,
        }
    }

    /// Add a parameter
    pub fn add_parameter(&mut self, param: Parameter) -> &mut Self {
        self.parameters.push(param);
        self
    }

    /// Builder method to add a parameter
    pub fn param(mut self, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        self.parameters.push(Parameter::new(name, data_type));
        self
    }

    /// Set the function body
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Function parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter name
    pub name: String,

    /// Parameter data type
    pub data_type: String,

    /// Optional default value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}

impl Parameter {
    /// Create a new parameter
    pub fn new(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            default_value: None,
        }
    }

    /// Set a default value
    pub fn default(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_builder() {
        let schema = Schema::with_database("SecurityDB")
            .table(
                Table::new("SecurityEvent")
                    .with_column("TimeGenerated", "datetime")
                    .with_column("Account", "string")
                    .with_column("EventID", "long")
                    .with_column("Computer", "string"),
            )
            .table(
                Table::new("SigninLogs")
                    .with_column("TimeGenerated", "datetime")
                    .with_column("UserPrincipalName", "string")
                    .with_column("IPAddress", "string")
                    .with_column("ResultType", "string"),
            );

        assert_eq!(schema.database, Some("SecurityDB".to_string()));
        assert_eq!(schema.tables.len(), 2);
        assert_eq!(schema.tables[0].columns.len(), 4);
    }

    #[test]
    fn test_schema_serialization() {
        let schema = Schema::new().table(
            Table::new("Test")
                .with_column("Id", "long")
                .with_column("Name", "string"),
        );

        let json = serde_json::to_string(&schema).unwrap();
        let parsed: Schema = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.tables.len(), 1);
        assert_eq!(parsed.tables[0].name, "Test");
        assert_eq!(parsed.tables[0].columns.len(), 2);
    }
}

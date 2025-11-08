use crate::client::{Client, QueryResponse, Table};
use crate::error::{KqlPanopticonError, Result};
use crate::workspace::Workspace;
use chrono::{DateTime, Local};
use log::{debug, info, warn};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Settings for query execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct QuerySettings {
    /// Base output folder for all results
    pub output_folder: PathBuf,

    /// Job name used for file naming
    pub job_name: String,

    /// Export results as CSV files
    pub export_csv: bool,

    /// Export results as JSON files
    pub export_json: bool,

    /// Parse nested dynamic fields into JSON objects (only affects JSON export)
    pub parse_dynamics: bool,
}

impl Default for QuerySettings {
    fn default() -> Self {
        Self {
            output_folder: PathBuf::from("./output"),
            job_name: "query".to_string(),
            export_csv: true,
            export_json: false,
            parse_dynamics: true,
        }
    }
}

impl QuerySettings {
    #[allow(dead_code)]
    pub fn new(output_folder: impl Into<PathBuf>, job_name: impl Into<String>) -> Self {
        Self {
            output_folder: output_folder.into(),
            job_name: job_name.into(),
            export_csv: true,
            export_json: false,
            parse_dynamics: true,
        }
    }

    pub fn with_formats(
        output_folder: impl Into<PathBuf>,
        job_name: impl Into<String>,
        export_csv: bool,
        export_json: bool,
        parse_dynamics: bool,
    ) -> Self {
        Self {
            output_folder: output_folder.into(),
            job_name: job_name.into(),
            export_csv,
            export_json,
            parse_dynamics,
        }
    }
}

/// Result of a single query job execution
#[derive(Debug, Clone)]
pub struct QueryJobResult {
    /// Workspace that was queried
    pub workspace_id: String,

    /// Workspace name
    pub workspace_name: String,

    /// Query that was executed
    pub query: String,

    /// Execution result
    pub result: Result<JobSuccess>,

    /// Time taken to execute
    pub elapsed: Duration,

    /// Timestamp when the job completed
    pub timestamp: DateTime<Local>,
}

/// Success information for a completed job
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobSuccess {
    /// Number of rows returned
    pub row_count: usize,

    /// Number of pages fetched (for paginated queries)
    #[allow(dead_code)]
    pub page_count: usize,

    /// Output file path
    pub output_path: PathBuf,

    /// File size in bytes
    pub file_size: u64,
}

/// Individual query job
struct QueryJob {
    workspace: Workspace,
    query: String,
    settings: QuerySettings,
    timestamp: String,
}

/// Helper for streaming CSV writes to a temporary file
struct StreamingCsvWriter {
    temp_path: PathBuf,
    file: tokio::fs::File,
    row_count: usize,
    page_count: usize,
    buffer: Vec<String>,
    buffer_size: usize,
}

impl StreamingCsvWriter {
    /// Create a new streaming CSV writer
    async fn new(temp_path: PathBuf, buffer_size: usize) -> Result<Self> {
        let file = tokio::fs::File::create(&temp_path).await?;
        Ok(Self {
            temp_path,
            file,
            row_count: 0,
            page_count: 0,
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
        })
    }

    /// Write CSV header
    async fn write_header(&mut self, table: &Table) -> Result<()> {
        let headers: Vec<String> = table.columns.iter().map(|col| col.name.clone()).collect();
        let header_line = format!("{}\n", headers.join(","));
        self.file.write_all(header_line.as_bytes()).await?;
        Ok(())
    }

    /// Add rows from a page to the buffer
    fn add_page(&mut self, table: &Table, format_fn: &impl Fn(&serde_json::Value) -> String) {
        self.page_count += 1;
        for row in &table.rows {
            if let Some(row_array) = row.as_array() {
                let row_strings: Vec<String> = row_array
                    .iter()
                    .map(format_fn)
                    .collect();
                self.buffer.push(format!("{}\n", row_strings.join(",")));
                self.row_count += 1;
            }
        }
    }

    /// Flush buffer to disk if it exceeds buffer_size
    async fn flush_if_needed(&mut self) -> Result<()> {
        if self.buffer.len() >= self.buffer_size {
            self.flush().await?;
        }
        Ok(())
    }

    /// Flush buffer to disk
    async fn flush(&mut self) -> Result<()> {
        if !self.buffer.is_empty() {
            let content = self.buffer.join("");
            self.file.write_all(content.as_bytes()).await?;
            self.buffer.clear();
        }
        Ok(())
    }

    /// Finalize the file and move to final location
    async fn finalize(mut self, final_path: &PathBuf) -> Result<usize> {
        // Flush any remaining buffered data
        self.flush().await?;

        // Ensure all data is written to disk
        self.file.sync_all().await?;

        // Close the file
        drop(self.file);

        // Move temp file to final location
        tokio::fs::rename(&self.temp_path, final_path).await?;

        Ok(self.row_count)
    }

    /// Clean up temp file on error
    async fn cleanup(self) -> Result<()> {
        drop(self.file);
        if self.temp_path.exists() {
            tokio::fs::remove_file(&self.temp_path).await?;
        }
        Ok(())
    }
}

/// Helper for streaming JSON writes to a temporary file
struct StreamingJsonWriter {
    temp_path: PathBuf,
    file: tokio::fs::File,
    row_count: usize,
    page_count: usize,
    buffer: Vec<serde_json::Value>,
    buffer_size: usize,
    table_columns: Option<Vec<crate::client::Column>>,
    parse_dynamics: bool,
}

impl StreamingJsonWriter {
    /// Create a new streaming JSON writer
    async fn new(temp_path: PathBuf, buffer_size: usize, parse_dynamics: bool) -> Result<Self> {
        let file = tokio::fs::File::create(&temp_path).await?;
        Ok(Self {
            temp_path,
            file,
            row_count: 0,
            page_count: 0,
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
            table_columns: None,
            parse_dynamics,
        })
    }

    /// Set table columns (must be called before adding pages)
    fn set_columns(&mut self, columns: Vec<crate::client::Column>) {
        self.table_columns = Some(columns);
    }

    /// Add rows from a page to the buffer
    fn add_page(&mut self, table: &Table) -> Result<()> {
        if self.table_columns.is_none() {
            return Err(KqlPanopticonError::InvalidConfiguration(
                "Table columns not set before adding page".to_string(),
            ));
        }

        self.page_count += 1;
        let columns = self.table_columns.as_ref().unwrap();

        for row in &table.rows {
            if let Some(row_array) = row.as_array() {
                let mut row_object = serde_json::Map::new();
                for (idx, value) in row_array.iter().enumerate() {
                    if let Some(column) = columns.get(idx) {
                        let processed_value = if self.parse_dynamics && column.column_type == "dynamic" {
                            Self::parse_dynamic_value(value)
                        } else {
                            value.clone()
                        };
                        row_object.insert(column.name.clone(), processed_value);
                    }
                }
                self.buffer.push(serde_json::Value::Object(row_object));
                self.row_count += 1;
            }
        }

        Ok(())
    }

    /// Flush buffer to disk if it exceeds buffer_size
    async fn flush_if_needed(&mut self) -> Result<()> {
        if self.buffer.len() >= self.buffer_size {
            self.flush().await?;
        }
        Ok(())
    }

    /// Flush buffer to disk (as newline-delimited JSON)
    async fn flush(&mut self) -> Result<()> {
        if !self.buffer.is_empty() {
            for value in &self.buffer {
                let line = serde_json::to_string(value)?;
                self.file.write_all(line.as_bytes()).await?;
                self.file.write_all(b"\n").await?;
            }
            self.buffer.clear();
        }
        Ok(())
    }

    /// Finalize the file and move to final location with metadata
    async fn finalize(
        mut self,
        final_path: &PathBuf,
        workspace: &Workspace,
        timestamp: &str,
        query: &str,
    ) -> Result<usize> {
        // Flush any remaining buffered data
        self.flush().await?;

        // Close the temp file
        drop(self.file);

        // Read all rows from temp file
        let temp_content = tokio::fs::read_to_string(&self.temp_path).await?;
        let rows: Vec<serde_json::Value> = temp_content
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).unwrap_or(serde_json::Value::Null))
            .collect();

        // Build final JSON with metadata
        let columns = self.table_columns.as_ref().ok_or_else(|| {
            KqlPanopticonError::InvalidConfiguration("Table columns not set".to_string())
        })?;

        let output = serde_json::json!({
            "metadata": {
                "workspace": workspace.name,
                "workspace_id": workspace.workspace_id,
                "subscription": workspace.subscription_name,
                "timestamp": timestamp,
                "query": query,
                "row_count": self.row_count,
                "page_count": self.page_count,
            },
            "columns": columns.iter().map(|col| {
                serde_json::json!({
                    "name": col.name,
                    "type": col.column_type,
                })
            }).collect::<Vec<_>>(),
            "rows": rows,
        });

        // Write final JSON to destination
        let json_content = serde_json::to_string_pretty(&output)?;
        tokio::fs::write(final_path, json_content).await?;

        // Clean up temp file
        tokio::fs::remove_file(&self.temp_path).await?;

        Ok(self.row_count)
    }

    /// Clean up temp file on error
    async fn cleanup(self) -> Result<()> {
        drop(self.file);
        if self.temp_path.exists() {
            tokio::fs::remove_file(&self.temp_path).await?;
        }
        Ok(())
    }

    /// Recursively parse dynamic values that might be JSON strings
    fn parse_dynamic_value(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                    Self::parse_dynamic_value(&parsed)
                } else {
                    value.clone()
                }
            }
            serde_json::Value::Array(arr) => {
                let processed: Vec<_> = arr.iter().map(Self::parse_dynamic_value).collect();
                serde_json::Value::Array(processed)
            }
            serde_json::Value::Object(obj) => {
                let mut processed = serde_json::Map::new();
                for (k, v) in obj {
                    processed.insert(k.clone(), Self::parse_dynamic_value(v));
                }
                serde_json::Value::Object(processed)
            }
            _ => value.clone(),
        }
    }
}

/// Builder for creating and executing query jobs
pub struct QueryJobBuilder {
    workspaces: Vec<Workspace>,
    queries: Vec<String>,
    settings: Option<QuerySettings>,
}

impl QueryJobBuilder {
    /// Create a new query job builder
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            queries: Vec::new(),
            settings: None,
        }
    }

    /// Add workspaces to query
    pub fn workspaces(mut self, workspaces: Vec<Workspace>) -> Self {
        self.workspaces = workspaces;
        self
    }

    /// Add queries to execute
    pub fn queries(mut self, queries: Vec<String>) -> Self {
        self.queries = queries;
        self
    }

    /// Set query settings
    pub fn settings(mut self, settings: QuerySettings) -> Self {
        self.settings = Some(settings);
        self
    }

    /// Generate timestamp string in format: YYYY-MM-DD_HH-MM-SS
    fn generate_timestamp() -> String {
        let now: DateTime<Local> = Local::now();
        now.format("%Y-%m-%d_%H-%M-%S").to_string()
    }

    /// Execute all query jobs
    pub async fn execute(self, client: &Client) -> Result<Vec<QueryJobResult>> {
        let settings = self.settings.ok_or_else(|| {
            KqlPanopticonError::InvalidConfiguration("QuerySettings not provided".to_string())
        })?;

        if self.workspaces.is_empty() {
            return Err(KqlPanopticonError::InvalidConfiguration(
                "No workspaces provided".to_string(),
            ));
        }

        if self.queries.is_empty() {
            return Err(KqlPanopticonError::InvalidConfiguration(
                "No queries provided".to_string(),
            ));
        }

        let timestamp = Self::generate_timestamp();

        // Create all jobs (cartesian product of workspaces � queries)
        let mut jobs = Vec::new();
        for workspace in self.workspaces {
            for query in &self.queries {
                jobs.push(QueryJob {
                    workspace: workspace.clone(),
                    query: query.clone(),
                    settings: settings.clone(),
                    timestamp: timestamp.clone(),
                });
            }
        }

        info!("Executing {} query job(s)", jobs.len());

        // Execute all jobs concurrently
        let mut tasks = Vec::new();
        for job in jobs {
            let client = client.clone();
            let task = tokio::spawn(async move { job.execute(&client).await });
            tasks.push(task);
        }

        // Collect results
        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!("Task panicked: {}", e);
                }
            }
        }

        Ok(results)
    }
}

impl Default for QueryJobBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryJob {
    /// Execute this query job
    async fn execute(self, client: &Client) -> QueryJobResult {
        let start = Instant::now();

        debug!(
            "Executing query on workspace '{}' ({})",
            self.workspace.name, self.workspace.workspace_id
        );

        let result = self.execute_and_save(client).await;
        let elapsed = start.elapsed();

        match &result {
            Ok(success) => {
                info!(
                    " Completed: {} rows written to {} ({:.2}s)",
                    success.row_count,
                    success.output_path.display(),
                    elapsed.as_secs_f64()
                );
            }
            Err(e) => {
                warn!(
                    " Failed on workspace '{}': {} ({:.2}s)",
                    self.workspace.name,
                    e,
                    elapsed.as_secs_f64()
                );
            }
        }

        QueryJobResult {
            workspace_id: self.workspace.workspace_id.clone(),
            workspace_name: self.workspace.name.clone(),
            query: self.query.clone(),
            result,
            elapsed,
            timestamp: Local::now(),
        }
    }

    /// Execute query and save to configured formats (CSV and/or JSON) with pagination support
    async fn execute_and_save(&self, client: &Client) -> Result<JobSuccess> {
        // Build output directory: output_folder/subscription_name/workspace_name/timestamp/
        let normalized_subscription = Workspace::normalize_name(&self.workspace.subscription_name);
        let normalized_workspace = Workspace::normalize_name(&self.workspace.name);

        let output_dir = self
            .settings
            .output_folder
            .join(normalized_subscription)
            .join(normalized_workspace)
            .join(&self.timestamp);

        // Create directory structure
        fs::create_dir_all(&output_dir).await?;

        let mut row_count = 0;
        let mut page_count = 0;
        let mut total_file_size = 0u64;
        let mut primary_output_path = None;

        // Export as CSV if enabled
        if self.settings.export_csv {
            let csv_path = output_dir.join(format!("{}.csv", self.settings.job_name));
            let (rows, pages) = self.write_csv_streaming(client, &csv_path).await?;
            row_count = rows;
            page_count = pages;
            let metadata = fs::metadata(&csv_path).await?;
            total_file_size += metadata.len();
            if primary_output_path.is_none() {
                primary_output_path = Some(csv_path);
            }
        }

        // Export as JSON if enabled
        if self.settings.export_json {
            let json_path = output_dir.join(format!("{}.json", self.settings.job_name));
            let (rows, pages) = self.write_json_streaming(client, &json_path).await?;
            row_count = rows;
            page_count = pages;
            let metadata = fs::metadata(&json_path).await?;
            total_file_size += metadata.len();
            if primary_output_path.is_none() {
                primary_output_path = Some(json_path);
            }
        }

        let output_path = primary_output_path.ok_or_else(|| {
            KqlPanopticonError::InvalidConfiguration(
                "No export format enabled (CSV or JSON required)".to_string(),
            )
        })?;

        Ok(JobSuccess {
            row_count,
            page_count,
            output_path,
            file_size: total_file_size,
        })
    }

    /// Write query response to CSV file with streaming and pagination
    async fn write_csv_streaming(&self, client: &Client, output_path: &PathBuf) -> Result<(usize, usize)> {
        // Create temp file path
        let temp_path = output_path.with_extension("tmp.csv");

        // Buffer 100 pages before flushing to disk (adjustable)
        const PAGE_BUFFER_SIZE: usize = 100;

        let mut writer = StreamingCsvWriter::new(temp_path.clone(), PAGE_BUFFER_SIZE).await?;

        // Execute first query with retry logic
        let timeout = client.query_timeout();
        let retry_count = client.retry_count();
        let mut response = self.execute_with_retry(client, timeout, retry_count).await?;

        if response.tables.is_empty() {
            writer.cleanup().await?;
            return Err(KqlPanopticonError::QueryExecutionFailed(
                "Query returned no tables".to_string(),
            ));
        }

        // Write header from first table
        let table = &response.tables[0];
        writer.write_header(table).await?;

        // Process first page
        writer.add_page(table, &|value| self.format_csv_value(value));
        writer.flush_if_needed().await?;

        // Follow pagination links
        while let Some(ref next_link) = response.next_link {
            debug!("Fetching next page: {} rows so far", writer.row_count);

            let page_future = client.query_next_page(next_link);
            response = match tokio::time::timeout(timeout, page_future).await {
                Ok(Ok(page)) => page,
                Ok(Err(e)) => {
                    // Pagination failed, cleanup and return error
                    writer.cleanup().await?;
                    return Err(e);
                }
                Err(_) => {
                    // Timeout, cleanup and return error
                    writer.cleanup().await?;
                    return Err(KqlPanopticonError::QueryExecutionFailed(
                        format!("Pagination request timed out after {} seconds", timeout.as_secs()),
                    ));
                }
            };

            if !response.tables.is_empty() {
                let table = &response.tables[0];
                writer.add_page(table, &|value| self.format_csv_value(value));
                writer.flush_if_needed().await?;
            }
        }

        // Finalize: flush remaining buffer and move to final location
        let row_count = writer.row_count;
        let page_count = writer.page_count;

        match writer.finalize(output_path).await {
            Ok(_) => Ok((row_count, page_count)),
            Err(e) => {
                // Try to cleanup temp file on finalization error
                let _ = tokio::fs::remove_file(&temp_path).await;
                Err(e)
            }
        }
    }

    /// Write query response to JSON file with streaming and pagination
    async fn write_json_streaming(&self, client: &Client, output_path: &PathBuf) -> Result<(usize, usize)> {
        // Create temp file path
        let temp_path = output_path.with_extension("tmp.json");

        // Buffer 100 pages before flushing to disk (adjustable)
        const PAGE_BUFFER_SIZE: usize = 100;

        let mut writer = StreamingJsonWriter::new(
            temp_path.clone(),
            PAGE_BUFFER_SIZE,
            self.settings.parse_dynamics,
        )
        .await?;

        // Execute first query with retry logic
        let timeout = client.query_timeout();
        let retry_count = client.retry_count();
        let mut response = self.execute_with_retry(client, timeout, retry_count).await?;

        if response.tables.is_empty() {
            writer.cleanup().await?;
            return Err(KqlPanopticonError::QueryExecutionFailed(
                "Query returned no tables".to_string(),
            ));
        }

        // Set columns from first table
        let table = &response.tables[0];
        writer.set_columns(table.columns.clone());

        // Process first page
        writer.add_page(table)?;
        writer.flush_if_needed().await?;

        // Follow pagination links
        while let Some(ref next_link) = response.next_link {
            debug!("Fetching next page: {} rows so far", writer.row_count);

            let page_future = client.query_next_page(next_link);
            response = match tokio::time::timeout(timeout, page_future).await {
                Ok(Ok(page)) => page,
                Ok(Err(e)) => {
                    // Pagination failed, cleanup and return error
                    writer.cleanup().await?;
                    return Err(e);
                }
                Err(_) => {
                    // Timeout, cleanup and return error
                    writer.cleanup().await?;
                    return Err(KqlPanopticonError::QueryExecutionFailed(
                        format!("Pagination request timed out after {} seconds", timeout.as_secs()),
                    ));
                }
            };

            if !response.tables.is_empty() {
                let table = &response.tables[0];
                writer.add_page(table)?;
                writer.flush_if_needed().await?;
            }
        }

        // Finalize: flush remaining buffer, wrap with metadata, and move to final location
        let row_count = writer.row_count;
        let page_count = writer.page_count;

        match writer
            .finalize(output_path, &self.workspace, &self.timestamp, &self.query)
            .await
        {
            Ok(_) => Ok((row_count, page_count)),
            Err(e) => {
                // Try to cleanup temp file on finalization error
                let _ = tokio::fs::remove_file(&temp_path).await;
                Err(e)
            }
        }
    }

    /// Execute query with retry logic and timeout
    async fn execute_with_retry(
        &self,
        client: &Client,
        timeout: Duration,
        retry_count: u32,
    ) -> Result<QueryResponse> {
        let mut last_error = None;
        let max_attempts = retry_count + 1; // retry_count of 0 means 1 attempt total

        for attempt in 0..max_attempts {
            if attempt > 0 {
                debug!(
                    "Retrying query on workspace '{}' (attempt {}/{})",
                    self.workspace.name,
                    attempt + 1,
                    max_attempts
                );
                // Exponential backoff: 1s, 2s, 4s, 8s, etc.
                let backoff = Duration::from_secs(2u64.pow(attempt - 1));
                tokio::time::sleep(backoff).await;
            }

            let query_future = client.query_workspace(&self.workspace.workspace_id, &self.query, None);
            match tokio::time::timeout(timeout, query_future).await {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(e)) => {
                    last_error = Some(e);
                }
                Err(_) => {
                    last_error = Some(KqlPanopticonError::QueryExecutionFailed(format!(
                        "Query timed out after {} seconds on workspace '{}'",
                        timeout.as_secs(),
                        self.workspace.name
                    )));
                }
            }
        }

        // All attempts failed
        Err(last_error.unwrap_or_else(|| {
            KqlPanopticonError::QueryExecutionFailed(format!(
                "Query failed on workspace '{}' after {} attempts",
                self.workspace.name, max_attempts
            ))
        }))
    }

    /// Format a JSON value for CSV output
    fn format_csv_value(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => String::new(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => {
                // Escape quotes and wrap in quotes if needed
                if s.contains(',') || s.contains('"') || s.contains('\n') {
                    format!("\"{}\"", s.replace('"', "\"\""))
                } else {
                    s.clone()
                }
            }
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                // Serialize complex types as JSON strings
                let json_str = value.to_string();
                format!("\"{}\"", json_str.replace('"', "\"\""))
            }
        }
    }
}

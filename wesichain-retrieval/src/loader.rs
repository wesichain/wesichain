use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use wesichain_core::{Document, Value};

use crate::error::IngestionError;

pub struct TextLoader {
    path: PathBuf,
}

impl TextLoader {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Vec<Document>, std::io::Error> {
        let content = fs::read_to_string(&self.path)?;
        let mut metadata = HashMap::new();
        metadata.insert(
            "source".to_string(),
            Value::String(self.path.to_string_lossy().to_string()),
        );

        Ok(vec![Document {
            id: self.path.to_string_lossy().to_string(),
            content,
            metadata,
            embedding: None,
        }])
    }
}

pub async fn load_file_async(path: PathBuf) -> Result<Vec<Document>, IngestionError> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| IngestionError::MissingExtension { path: path.clone() })?;

    match extension.as_str() {
        "txt" => load_text_file_async(path).await,
        _ => Err(IngestionError::UnsupportedExtension { path, extension }),
    }
}

pub async fn load_files_async(paths: Vec<PathBuf>) -> Result<Vec<Document>, IngestionError> {
    let mut documents = Vec::new();
    for path in paths {
        documents.extend(load_file_async(path).await?);
    }
    Ok(documents)
}

async fn load_text_file_async(path: PathBuf) -> Result<Vec<Document>, IngestionError> {
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|source| IngestionError::Read {
            path: path.clone(),
            source,
        })?;

    let mut metadata = HashMap::new();
    metadata.insert(
        "source".to_string(),
        Value::String(path.to_string_lossy().to_string()),
    );

    Ok(vec![Document {
        id: path.to_string_lossy().to_string(),
        content,
        metadata,
        embedding: None,
    }])
}

#[cfg(feature = "pdf")]
pub struct PdfLoader {
    path: PathBuf,
}

#[cfg(feature = "pdf")]
impl PdfLoader {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Vec<Document>, std::io::Error> {
        let content = pdf_extract::extract_text(&self.path).map_err(std::io::Error::other)?;
        let mut metadata = HashMap::new();
        metadata.insert(
            "source".to_string(),
            Value::String(self.path.to_string_lossy().to_string()),
        );

        Ok(vec![Document {
            id: self.path.to_string_lossy().to_string(),
            content,
            metadata,
            embedding: None,
        }])
    }
}

#[cfg(not(feature = "pdf"))]
pub struct PdfLoader {
    path: PathBuf,
}

#[cfg(not(feature = "pdf"))]
impl PdfLoader {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Vec<Document>, std::io::Error> {
        let _ = &self.path;
        Err(std::io::Error::other("pdf feature disabled"))
    }
}

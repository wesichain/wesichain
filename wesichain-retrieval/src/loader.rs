use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

use quick_xml::events::Event;
use quick_xml::Reader;
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
        "docx" => load_docx_file_async(path).await,
        "html" | "htm" => load_html_file_async(path).await,
        "md" | "markdown" => load_markdown_file_async(path).await,
        #[cfg(feature = "pdf")]
        "pdf" => load_pdf_file_async(path).await,
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
    let content =
        tokio::fs::read_to_string(&path)
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

async fn load_docx_file_async(path: PathBuf) -> Result<Vec<Document>, IngestionError> {
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|source| IngestionError::Read {
            path: path.clone(),
            source,
        })?;

    let content = parse_docx_text(&bytes).map_err(|source| IngestionError::Parse {
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

async fn load_html_file_async(path: PathBuf) -> Result<Vec<Document>, IngestionError> {
    let html_content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|source| IngestionError::Read {
            path: path.clone(),
            source,
        })?;

    let document = scraper::Html::parse_document(&html_content);
    
    // Extract title
    let title_selector = scraper::Selector::parse("title").unwrap();
    let title = document
        .select(&title_selector)
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_default();

    // Extract lang attribute
    let html_selector = scraper::Selector::parse("html").unwrap();
    let lang = document
        .select(&html_selector)
        .next()
        .and_then(|el| el.value().attr("lang"))
        .map(String::from);

    // Extract text from body, skipping script/style/nav/header/footer
    let body_selector = scraper::Selector::parse("body").unwrap();
    
    let mut text_parts = Vec::new();
    
    if let Some(body) = document.select(&body_selector).next() {
        extract_text_from_html_element(body, &mut text_parts);
    } else {
        // Fallback: extract from entire document if no body
        extract_text_from_html_element(document.root_element(), &mut text_parts);
    }

    let content = text_parts.join("\n\n").trim().to_string();

    let mut metadata = HashMap::new();
    metadata.insert(
        "source".to_string(),
        Value::String(path.to_string_lossy().to_string()),
    );
    if !title.is_empty() {
        metadata.insert("title".to_string(), Value::String(title));
    }
    if let Some(lang_value) = lang {
        metadata.insert("lang".to_string(), Value::String(lang_value));
    }

    Ok(vec![Document {
        id: path.to_string_lossy().to_string(),
        content,
        metadata,
        embedding: None,
    }])
}

fn extract_text_from_html_element(
    element: scraper::ElementRef,
    text_parts: &mut Vec<String>,
) {
    use scraper::node::Node;
    
    let mut current_text = String::new();
    
    for child in element.children() {
        match child.value() {
            Node::Element(_) => {
                if let Some(child_element) = scraper::ElementRef::wrap(child) {
                    // Skip script, style, nav, header, footer
                    match child_element.value().name() {
                        "script" | "style" | "nav" | "header" | "footer" => continue,
                        _ => {}
                    }
                    
                    // Recursively extract from children
                    extract_text_from_html_element(child_element, text_parts);
                    
                    // Add paragraph break after block elements
                    if matches!(
                        child_element.value().name(),
                        "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" 
                        | "li" | "blockquote" | "pre" | "br"
                    ) {
                        if !current_text.is_empty() {
                            text_parts.push(std::mem::take(&mut current_text).trim().to_string());
                        }
                    }
                }
            }
            Node::Text(text) => {
                let text_content = text.trim();
                if !text_content.is_empty() {
                    if !current_text.is_empty() && !current_text.ends_with(' ') {
                        current_text.push(' ');
                    }
                    current_text.push_str(text_content);
                }
            }
            _ => {}
        }
    }
    
    if !current_text.is_empty() {
        text_parts.push(current_text.trim().to_string());
    }
}

/// Load a Markdown file and extract content with header metadata.
///
/// # Metadata Schema
///
/// The loader preserves header structure in the `metadata` field for future use
/// by text splitters like `MarkdownHeaderTextSplitter`.
///
/// ## `headers` Field
///
/// An array of header objects with the following schema:
///
/// ```json
/// {
///   "headers": [
///     {
///       "level": 1,
///       "text": "Main Title",
///       "line_start": 1
///     },
///     {
///       "level": 2,
///       "text": "Subsection",
///       "line_start": 15
///     }
///   ]
/// }
/// ```
///
/// - `level`: Header level (1-6)
/// - `text`: Header text content (without leading `#` symbols)
/// - `line_start`: Approximate line number where the header appears
///
/// ## Example Usage
///
/// ```rust,ignore
/// use wesichain_retrieval::load_file_async;
/// use std::path::PathBuf;
///
/// let docs = load_file_async(PathBuf::from("readme.md")).await?;
/// let headers = docs[0].metadata.get("headers").unwrap();
/// // Access header hierarchy for splitting or filtering
/// ```
async fn load_markdown_file_async(path: PathBuf) -> Result<Vec<Document>, IngestionError> {
    let markdown_content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|source| IngestionError::Read {
            path: path.clone(),
            source,
        })?;

    use pulldown_cmark::{Event, Parser, Tag, TagEnd, HeadingLevel};
    
    let parser = Parser::new(&markdown_content);
    
    let mut text_parts = Vec::new();
    let mut headers = Vec::new();
    let mut current_text = String::new();
    let mut line_number = 0;
    
    for event in parser {
        match event {
            Event::Start(Tag::Heading { level: _, .. }) => {
                if !current_text.is_empty() {
                    text_parts.push(std::mem::take(&mut current_text));
                }
            }
            Event::End(TagEnd::Heading(level)) => {
                let header_text = current_text.trim().to_string();
                if !header_text.is_empty() {
                    let level_num = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    
                    let mut header_meta = HashMap::new();
                    header_meta.insert("level".to_string(), Value::Number(serde_json::Number::from(level_num)));
                    header_meta.insert("text".to_string(), Value::String(header_text.clone()));
                    header_meta.insert("line_start".to_string(), Value::Number(serde_json::Number::from(line_number)));
                    
                    headers.push(Value::Object(header_meta.into_iter().map(|(k, v)| (k, v)).collect()));
                    text_parts.push(header_text);
                    current_text.clear();
                }
            }
            Event::Text(text) | Event::Code(text) => {
                current_text.push_str(&text);
                line_number += text.matches('\n').count();
            }
            Event::SoftBreak | Event::HardBreak => {
                current_text.push(' ');
                line_number += 1;
            }
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => {
                if !current_text.is_empty() {
                    text_parts.push(std::mem::take(&mut current_text).trim().to_string());
                }
            }
            _ => {}
        }
    }
    
    if !current_text.is_empty() {
        text_parts.push(current_text.trim().to_string());
    }

    let content = text_parts.join("\n\n");

    let mut metadata = HashMap::new();
    metadata.insert(
        "source".to_string(),
        Value::String(path.to_string_lossy().to_string()),
    );
    if !headers.is_empty() {
        metadata.insert("headers".to_string(), Value::Array(headers));
    }

    Ok(vec![Document {
        id: path.to_string_lossy().to_string(),
        content,
        metadata,
        embedding: None,
    }])
}


#[cfg(feature = "pdf")]
async fn load_pdf_file_async(path: PathBuf) -> Result<Vec<Document>, IngestionError> {
    let loader = PdfLoader::new(path.clone());
    loader.load().map_err(|source| match source.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied => {
            IngestionError::Read { path, source }
        }
        _ => IngestionError::Parse { path, source },
    })
}

fn parse_docx_text(bytes: &[u8]) -> Result<String, std::io::Error> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(invalid_docx)?;

    let mut document_xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(invalid_docx)?
        .read_to_string(&mut document_xml)
        .map_err(invalid_docx)?;

    extract_docx_plain_text(&document_xml)
}

fn extract_docx_plain_text(xml: &str) -> Result<String, std::io::Error> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut blocks = Vec::new();
    let mut paragraph = String::new();
    let mut cell = String::new();
    let mut row = Vec::new();
    let mut table_rows = Vec::new();
    let mut inside_table = false;
    let mut inside_paragraph = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => match local_name(element.name().as_ref()) {
                b"tbl" => {
                    inside_table = true;
                    table_rows.clear();
                }
                b"tr" if inside_table => {
                    row.clear();
                }
                b"tc" if inside_table => {
                    cell.clear();
                }
                b"p" => {
                    inside_paragraph = true;
                    paragraph.clear();
                }
                _ => {}
            },
            Ok(Event::Text(text)) => {
                if inside_paragraph {
                    let value = text
                        .decode()
                        .map_err(|error| invalid_docx(format!("text decode error: {error}")))?;
                    append_docx_text_segment(&mut paragraph, &value);
                }
            }
            Ok(Event::CData(text)) => {
                if inside_paragraph {
                    let value = text
                        .decode()
                        .map_err(|error| invalid_docx(format!("cdata decode error: {error}")))?;
                    append_docx_text_segment(&mut paragraph, &value);
                }
            }
            Ok(Event::End(element)) => match local_name(element.name().as_ref()) {
                b"p" => {
                    let value = normalize_whitespace(&paragraph);
                    if !value.is_empty() {
                        if inside_table {
                            if !cell.is_empty() {
                                cell.push(' ');
                            }
                            cell.push_str(&value);
                        } else {
                            blocks.push(value);
                        }
                    }
                    inside_paragraph = false;
                    paragraph.clear();
                }
                b"tc" if inside_table => {
                    row.push(cell.trim().to_string());
                    cell.clear();
                }
                b"tr" if inside_table => {
                    if !row.is_empty() {
                        table_rows.push(std::mem::take(&mut row));
                    }
                }
                b"tbl" if inside_table => {
                    let table_text = format_table_rows(&table_rows);
                    if !table_text.is_empty() {
                        blocks.push(table_text);
                    }
                    table_rows.clear();
                    inside_table = false;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(invalid_docx(format!("xml parse error: {error}")));
            }
            _ => {}
        }
    }

    Ok(blocks.join("\n\n"))
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn append_docx_text_segment(paragraph: &mut String, value: &str) {
    if value.trim().is_empty() {
        return;
    }

    paragraph.push_str(value);
}

fn format_table_rows(rows: &[Vec<String>]) -> String {
    rows.iter()
        .filter(|row| row.iter().any(|cell| !cell.is_empty()))
        .map(|row| row.join(" | "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|byte| *byte == b':') {
        Some(index) => &name[index + 1..],
        None => name,
    }
}

fn invalid_docx<E: std::fmt::Display>(error: E) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("invalid docx: {error}"),
    )
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

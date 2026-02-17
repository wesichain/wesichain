use wesichain_graph::GraphError;

pub fn safe_thread_id(thread_id: &str) -> Result<&str, GraphError> {
    if thread_id.is_empty() {
        return Err(GraphError::Checkpoint(
            "thread_id must not be empty".to_string(),
        ));
    }

    if thread_id
        .chars()
        .any(|c| matches!(c, '{' | '}' | '*' | '?' | '\n' | '\r'))
    {
        return Err(GraphError::Checkpoint(format!(
            "thread_id contains characters invalid in Redis keys: {thread_id:?}"
        )));
    }

    Ok(thread_id)
}

#[derive(Debug, Clone)]
pub struct ThreadKeys {
    pub seq: String,
    pub latest: String,
    pub hist_prefix: String,
}

impl ThreadKeys {
    pub fn new(namespace: &str, thread_id: &str) -> Self {
        let tag = format!("{{cp:{namespace}:{thread_id}}}");
        Self {
            seq: format!("{tag}:seq"),
            latest: format!("{tag}:latest"),
            hist_prefix: format!("{tag}:hist"),
        }
    }
}

pub fn index_key(namespace: &str) -> String {
    format!("cp:{namespace}:index")
}

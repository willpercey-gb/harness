use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

// ---------------------------------------------------------------------------
// Parameter types — mirror the JSON schema the harness bridge expects.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryKnowledgeParams {
    /// Natural language query — hybrid vector + full-text + graph search.
    pub query: String,
    /// Max results (default 10, max 50).
    pub limit: Option<usize>,
    /// Filter by entity types: person, organization, project, technology, topic, location, component.
    pub entity_types: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetEntityParams {
    /// Entity type: person, organization, project, technology, topic, location, component.
    pub entity_type: String,
    /// Entity name.
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetEntityByNameParams {
    /// Entity name (case-insensitive, all types).
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetEntityGraphParams {
    /// Entity name.
    pub name: String,
    /// Optional entity type to disambiguate.
    pub entity_type: Option<String>,
    /// Number of relationship hops (default 2, max 3).
    pub depth: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListEntitiesParams {
    pub entity_type: String,
    pub filter: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AddEntityParams {
    pub entity_type: String,
    pub name: String,
    pub description: Option<String>,
    pub content: Option<String>,
    pub aliases: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateEntityParams {
    /// Existing entity name (case-insensitive).
    pub name: String,
    /// Replaces the existing description.
    pub description: Option<String>,
    /// Markdown bullets — appended to existing content.
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AddRelationshipParams {
    /// Source entity name.
    pub from_name: String,
    /// Optional source type to disambiguate.
    pub from_type: Option<String>,
    /// works_at | part_of | works_on | uses_tech | knows_about | related_to | mentions
    pub rel_type: String,
    /// Target entity name.
    pub to_name: String,
    /// Optional target type.
    pub to_type: Option<String>,
    /// Optional metadata.
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AddMemoryParams {
    pub content: String,
    pub summary: Option<String>,
    /// Defaults to "manual" — used by the Claude plugin.
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub source_path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SearchMemoriesParams {
    pub query: String,
    pub source_type: Option<String>,
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Bridge MCP server.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct HarnessMcpServer {
    port: u16,
    #[allow(dead_code)]
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

async fn bridge_call(port: u16, method: &str, params: serde_json::Value) -> Result<String, String> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .map_err(|e| format!("Bridge connection failed (is harness running?): {e}"))?;
    let req = serde_json::json!({ "method": method, "params": params });
    let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("Bridge write failed: {e}"))?;
    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .await
        .map_err(|e| format!("Bridge read failed: {e}"))?;
    let resp: serde_json::Value = serde_json::from_str(&response_line)
        .map_err(|e| format!("Bridge response parse failed: {e}"))?;
    if let Some(err) = resp.get("error").and_then(|e| e.as_str()) {
        return Err(err.to_string());
    }
    match resp.get("result") {
        Some(r) => serde_json::to_string_pretty(r).map_err(|e| e.to_string()),
        None => Ok("null".to_string()),
    }
}

#[tool_router]
impl HarnessMcpServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "query_knowledge",
        description = "Hybrid search across the harness knowledge graph: vector similarity, BM25, entity matching, and recency. Start here for any user question that might already be answered in their stored knowledge."
    )]
    async fn query_knowledge(
        &self,
        Parameters(p): Parameters<QueryKnowledgeParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "query_knowledge", serde_json::to_value(p).unwrap()).await
    }

    #[tool(
        name = "search_memories",
        description = "Vector search over stored memory chunks. Use alongside query_knowledge for broader coverage."
    )]
    async fn search_memories(
        &self,
        Parameters(p): Parameters<SearchMemoriesParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "search_memories", serde_json::to_value(p).unwrap()).await
    }

    #[tool(
        name = "get_entity",
        description = "Look up an entity by type and name. Returns description, content (knowledge bullets), aliases. Use get_entity_by_name if you don't know the type."
    )]
    async fn get_entity(
        &self,
        Parameters(p): Parameters<GetEntityParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "get_entity", serde_json::to_value(p).unwrap()).await
    }

    #[tool(
        name = "get_entity_by_name",
        description = "Look up an entity by name only (case-insensitive, all types). Returns the entity plus its detected type."
    )]
    async fn get_entity_by_name(
        &self,
        Parameters(p): Parameters<GetEntityByNameParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "get_entity_by_name", serde_json::to_value(p).unwrap()).await
    }

    #[tool(
        name = "get_entity_graph",
        description = "Get an entity and its connected neighbours within N hops. Shows works_at / works_on / uses_tech / etc."
    )]
    async fn get_entity_graph(
        &self,
        Parameters(p): Parameters<GetEntityGraphParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "get_entity_graph", serde_json::to_value(p).unwrap()).await
    }

    #[tool(
        name = "list_entities",
        description = "List entities of a type, optionally filtered by name substring. Use to browse the graph or find near-matches before adding new ones."
    )]
    async fn list_entities(
        &self,
        Parameters(p): Parameters<ListEntitiesParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "list_entities", serde_json::to_value(p).unwrap()).await
    }

    #[tool(
        name = "add_entity",
        description = "Create or upsert an entity (case-insensitive name dedup). ALWAYS include a description (what it IS); use update_entity afterwards to add content bullets. Types: person, organization, project, technology, topic, location, component."
    )]
    async fn add_entity(
        &self,
        Parameters(p): Parameters<AddEntityParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "add_entity", serde_json::to_value(p).unwrap()).await
    }

    #[tool(
        name = "update_entity",
        description = "Update an existing entity. description replaces existing; content is APPENDED. Always read with get_entity_by_name first to see current state."
    )]
    async fn update_entity(
        &self,
        Parameters(p): Parameters<UpdateEntityParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "update_entity", serde_json::to_value(p).unwrap()).await
    }

    #[tool(
        name = "add_relationship",
        description = "Create a typed edge between two entities. Both must already exist. rel_type: works_at | part_of | works_on | uses_tech | knows_about | related_to | mentions."
    )]
    async fn add_relationship(
        &self,
        Parameters(p): Parameters<AddRelationshipParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "add_relationship", serde_json::to_value(p).unwrap()).await
    }

    #[tool(
        name = "add_memory",
        description = "Store a free-form memory chunk. For entity-specific facts prefer update_entity. Source defaults to 'manual'."
    )]
    async fn add_memory(
        &self,
        Parameters(p): Parameters<AddMemoryParams>,
    ) -> Result<String, String> {
        bridge_call(self.port, "add_memory", serde_json::to_value(p).unwrap()).await
    }
}

#[tool_handler]
impl ServerHandler for HarnessMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Harness Knowledge Base — search and write to the user's personal graph + memories. \
             query_knowledge for hybrid search; get_entity_by_name for entity lookup; \
             add_entity / update_entity / add_relationship / add_memory to write.",
        )
    }
}

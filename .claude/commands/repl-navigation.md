# REPL Navigation Commands Implementation

Implement navigation and view control commands for the REPL.

## Files to Create/Modify

1. `rust/crates/ob-poc-ui/src/repl/commands/mod.rs`
2. `rust/crates/ob-poc-ui/src/repl/commands/navigation.rs`
3. `rust/crates/ob-poc-ui/src/repl/commands/view.rs`

## Navigation Commands

### navigation.rs
```rust
use std::sync::Arc;

/// Navigation command handlers
pub struct NavigationCommands {
    session: Arc<SessionManager>,
    graph_loader: Arc<dyn GraphLoader>,
}

impl NavigationCommands {
    pub fn new(session: Arc<SessionManager>, graph_loader: Arc<dyn GraphLoader>) -> Self {
        Self { session, graph_loader }
    }
    
    /// cd <path> - Navigate to scope
    /// Supports: absolute (allianz.trading.germany), relative (../trading, ..)
    pub async fn cd(&self, path: &str) -> Result<String, CommandError> {
        let current = self.session.current();
        
        let new_scope = if path == ".." {
            // Navigate up
            current.scope.parent()
                .ok_or(CommandError::AlreadyAtRoot)?
        } else if path.starts_with("..") {
            // Relative path going up
            let mut scope = current.scope.clone();
            for segment in path.split('/') {
                if segment == ".." {
                    scope = scope.parent()
                        .ok_or(CommandError::AlreadyAtRoot)?;
                } else if !segment.is_empty() {
                    scope = self.resolve_child_scope(&scope, segment).await?;
                }
            }
            scope
        } else if path.starts_with('/') || path.contains('.') {
            // Absolute path
            ScopePath::from_string(path)
        } else {
            // Relative path going down
            self.resolve_child_scope(&current.scope, path).await?
        };
        
        // Load graph for new scope
        let graph = self.graph_loader.load_scope(&new_scope).await?;
        
        self.session.navigate(new_scope.clone(), graph);
        
        Ok(format!(
            "Navigated to: {}\nMass: {} | {} CBUs, {} persons",
            new_scope.display(),
            self.session.current().mass.total,
            self.session.current().mass.breakdown.cbus,
            self.session.current().mass.breakdown.persons,
        ))
    }
    
    async fn resolve_child_scope(&self, parent: &ScopePath, child: &str) -> Result<ScopePath, CommandError> {
        // Look for entity in current scope that matches the name
        let current = self.session.current();
        
        let entity = current.graph.nodes.iter()
            .find(|n| n.name.eq_ignore_ascii_case(child) || 
                      n.id.to_string() == child)
            .ok_or_else(|| CommandError::EntityNotFound(child.to_string()))?;
        
        let segment = ScopeSegment {
            name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            entity_id: Some(entity.id),
            mass: 0,  // Will be computed after load
        };
        
        Ok(parent.push(segment))
    }
    
    /// pwd - Print current scope with mass
    pub fn pwd(&self) -> String {
        let ctx = self.session.current();
        format!(
            r#"
┌─────────────────────────────────────────────────────────────────┐
│ SCOPE: {}
│ MASS:  {} ({} CBUs, {} persons, {} holdings, {} floating)
│ DEPTH: {}
│ VIEW:  {:?}
└─────────────────────────────────────────────────────────────────┘
"#,
            ctx.scope.display(),
            ctx.mass.total,
            ctx.mass.breakdown.cbus,
            ctx.mass.breakdown.persons,
            ctx.mass.breakdown.holdings,
            ctx.mass.breakdown.floating,
            ctx.scope.depth(),
            ctx.view_mode,
        )
    }
    
    /// ls - List entities in current scope
    pub fn ls(&self, args: Option<&str>) -> String {
        let ctx = self.session.current();
        let mut output = String::new();
        
        // Parse optional filter
        let filter_type = args.and_then(|a| {
            if a.starts_with("-t") {
                a.strip_prefix("-t").map(|s| s.trim())
            } else {
                None
            }
        });
        
        // Group by entity type
        let mut by_type: std::collections::HashMap<String, Vec<&Node>> = std::collections::HashMap::new();
        
        for node in &ctx.graph.nodes {
            if let Some(ft) = filter_type {
                if !node.entity_type.eq_ignore_ascii_case(ft) {
                    continue;
                }
            }
            by_type.entry(node.entity_type.clone())
                .or_default()
                .push(node);
        }
        
        output.push_str(&format!("Contents of [{}]:\n\n", ctx.scope.display()));
        
        for (entity_type, nodes) in by_type.iter() {
            output.push_str(&format!("── {} ({}) ──\n", entity_type, nodes.len()));
            
            for node in nodes.iter().take(20) {  // Limit to first 20
                let drill = if node.entity_type == "CBU" { " ▶" } else { "" };
                output.push_str(&format!("  {} {}{}\n", node.id, node.name, drill));
            }
            
            if nodes.len() > 20 {
                output.push_str(&format!("  ... and {} more\n", nodes.len() - 20));
            }
            output.push('\n');
        }
        
        output.push_str(&format!("Total: {} entities\n", ctx.graph.nodes.len()));
        
        output
    }
    
    /// focus <entity_id> - Set focal entity
    pub fn focus(&self, entity_id: &str) -> Result<String, CommandError> {
        let id = parse_entity_id(entity_id)?;
        
        let ctx = self.session.current();
        let entity = ctx.graph.nodes.iter()
            .find(|n| n.id == id)
            .ok_or_else(|| CommandError::EntityNotFound(entity_id.to_string()))?;
        
        self.session.set_focus(Some(id));
        
        Ok(format!("Focus set to: {} ({})", entity.name, entity.entity_type))
    }
    
    /// select <entity_ids...> - Multi-select for batch operations
    pub fn select(&self, ids: &[&str]) -> Result<String, CommandError> {
        let mut entity_ids = vec![];
        
        for id_str in ids {
            let id = parse_entity_id(id_str)?;
            entity_ids.push(id);
        }
        
        let mut ctx = self.session.current();
        ctx.selected = entity_ids.clone();
        // Note: This needs proper mutation through session manager
        
        Ok(format!("Selected {} entities", entity_ids.len()))
    }
    
    /// filter <expr> - Apply filter to current view
    pub fn filter(&self, expr: &str) -> Result<String, CommandError> {
        let filter = parse_filter_expr(expr)?;
        self.session.apply_filter(filter.clone());
        
        let ctx = self.session.current();
        let matching = ctx.graph.nodes.iter()
            .filter(|n| filter.matches(n))
            .count();
        
        Ok(format!(
            "Filter applied: {}\nMatching entities: {} / {}",
            expr,
            matching,
            ctx.graph.nodes.len()
        ))
    }
    
    /// clear-filter - Remove all filters
    pub fn clear_filter(&self) -> String {
        self.session.clear_filters();
        "Filters cleared".to_string()
    }
}

fn parse_entity_id(s: &str) -> Result<EntityId, CommandError> {
    s.parse::<u64>()
        .map(EntityId)
        .map_err(|_| CommandError::InvalidEntityId(s.to_string()))
}

fn parse_filter_expr(expr: &str) -> Result<Filter, CommandError> {
    // Simple filter parsing: "role=UBO" or "type=CBU"
    if let Some((key, value)) = expr.split_once('=') {
        match key.to_lowercase().as_str() {
            "role" => Ok(Filter::ByRole(vec![value.to_string()])),
            "type" => Ok(Filter::ByEntityType(vec![value.to_string()])),
            _ => Ok(Filter::ByAttribute {
                key: key.to_string(),
                value: value.to_string(),
            }),
        }
    } else {
        Err(CommandError::InvalidFilterExpr(expr.to_string()))
    }
}

#[derive(Debug)]
pub enum CommandError {
    AlreadyAtRoot,
    EntityNotFound(String),
    InvalidEntityId(String),
    InvalidFilterExpr(String),
    LoadError(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyAtRoot => write!(f, "Already at root scope"),
            Self::EntityNotFound(e) => write!(f, "Entity not found: {}", e),
            Self::InvalidEntityId(e) => write!(f, "Invalid entity ID: {}", e),
            Self::InvalidFilterExpr(e) => write!(f, "Invalid filter expression: {}", e),
            Self::LoadError(e) => write!(f, "Failed to load: {}", e),
        }
    }
}
```

## View Commands

### view.rs
```rust
/// View control command handlers
pub struct ViewCommands {
    session: Arc<SessionManager>,
    view_controller: Arc<RwLock<ViewModeController>>,
}

impl ViewCommands {
    pub fn new(
        session: Arc<SessionManager>,
        view_controller: Arc<RwLock<ViewModeController>>,
    ) -> Self {
        Self { session, view_controller }
    }
    
    /// view - Show current view mode and reason
    pub fn view(&self) -> String {
        let ctx = self.session.current();
        let controller = self.view_controller.read();
        
        let reason = match ctx.view_mode {
            ViewMode::AstroOverview => {
                format!("Mass {} > astro threshold", ctx.mass.total)
            }
            ViewMode::HybridDrilldown => {
                format!("Mass {} in hybrid range", ctx.mass.total)
            }
            ViewMode::MultiCbuDetail => {
                format!("{} CBUs visible", ctx.mass.breakdown.cbus)
            }
            ViewMode::SingleCbuPyramid => {
                "Single CBU in scope".to_string()
            }
            ViewMode::FullDetail => {
                "Full detail mode".to_string()
            }
            _ => "Unknown".to_string()
        };
        
        let transition_status = if controller.pending_transition.is_some() {
            " (transitioning...)"
        } else {
            ""
        };
        
        format!(
            r#"
View Mode: {:?}{}
Reason: {}

Available modes:
  astro    - Solar system overview (for large datasets)
  pyramid  - Hierarchical UBO structure
  matrix   - Grid layout by attributes
  hybrid   - CBUs with embedded pyramids
  auto     - Automatic based on mass
"#,
            ctx.view_mode,
            transition_status,
            reason,
        )
    }
    
    /// view <mode> - Force view mode
    pub fn view_set(&self, mode: &str) -> Result<String, CommandError> {
        let view_mode = match mode.to_lowercase().as_str() {
            "astro" | "astro_overview" => ViewMode::AstroOverview,
            "pyramid" | "single_cbu_pyramid" => ViewMode::SingleCbuPyramid,
            "matrix" => ViewMode::Matrix,
            "hybrid" | "hybrid_drilldown" => ViewMode::HybridDrilldown,
            "detail" | "full_detail" => ViewMode::FullDetail,
            "auto" => {
                // Re-enable automatic mode selection
                let mut controller = self.view_controller.write();
                controller.auto_mode = true;
                return Ok("View mode set to AUTO (mass-based selection)".to_string());
            }
            _ => return Err(CommandError::InvalidViewMode(mode.to_string())),
        };
        
        {
            let mut controller = self.view_controller.write();
            controller.auto_mode = false;
            controller.force_mode(view_mode.clone());
        }
        
        Ok(format!("View mode forced to: {:?}", view_mode))
    }
    
    /// zoom <level> - Set viewport zoom level
    pub fn zoom(&self, level: &str) -> Result<String, CommandError> {
        let zoom: f32 = level.parse()
            .map_err(|_| CommandError::InvalidZoom(level.to_string()))?;
        
        if zoom < 0.1 || zoom > 10.0 {
            return Err(CommandError::ZoomOutOfRange);
        }
        
        // Send zoom command to viewport
        // This would be via a channel or shared state
        
        Ok(format!("Zoom set to: {}x", zoom))
    }
    
    /// center [entity_id] - Center viewport on entity
    pub fn center(&self, entity_id: Option<&str>) -> Result<String, CommandError> {
        let ctx = self.session.current();
        
        let target = if let Some(id_str) = entity_id {
            let id = parse_entity_id(id_str)?;
            ctx.graph.nodes.iter()
                .find(|n| n.id == id)
                .ok_or_else(|| CommandError::EntityNotFound(id_str.to_string()))?
        } else if let Some(focal) = ctx.focal_entity {
            ctx.graph.nodes.iter()
                .find(|n| n.id == focal)
                .ok_or(CommandError::NoFocalEntity)?
        } else {
            return Err(CommandError::NoFocalEntity);
        };
        
        // Send center command to viewport
        
        Ok(format!("Viewport centered on: {}", target.name))
    }
}

impl CommandError {
    // Add to existing enum
}

#[derive(Debug)]
pub enum CommandError {
    // ... existing variants
    InvalidViewMode(String),
    InvalidZoom(String),
    ZoomOutOfRange,
    NoFocalEntity,
}
```

## Command Router

### mod.rs
```rust
pub mod navigation;
pub mod view;

use navigation::NavigationCommands;
use view::ViewCommands;

pub struct CommandRouter {
    navigation: NavigationCommands,
    view: ViewCommands,
}

impl CommandRouter {
    pub fn new(
        session: Arc<SessionManager>,
        graph_loader: Arc<dyn GraphLoader>,
        view_controller: Arc<RwLock<ViewModeController>>,
    ) -> Self {
        Self {
            navigation: NavigationCommands::new(session.clone(), graph_loader),
            view: ViewCommands::new(session, view_controller),
        }
    }
    
    pub async fn execute(&self, input: &str) -> String {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        
        if parts.is_empty() {
            return String::new();
        }
        
        let command = parts[0];
        let args = &parts[1..];
        
        match command {
            // Navigation commands
            "cd" => {
                if args.is_empty() {
                    "Usage: cd <path>".to_string()
                } else {
                    match self.navigation.cd(args[0]).await {
                        Ok(msg) => msg,
                        Err(e) => format!("Error: {}", e),
                    }
                }
            }
            "pwd" => self.navigation.pwd(),
            "ls" => self.navigation.ls(args.first().copied()),
            "focus" => {
                if args.is_empty() {
                    "Usage: focus <entity_id>".to_string()
                } else {
                    match self.navigation.focus(args[0]) {
                        Ok(msg) => msg,
                        Err(e) => format!("Error: {}", e),
                    }
                }
            }
            "select" => {
                if args.is_empty() {
                    "Usage: select <entity_id> [entity_id...]".to_string()
                } else {
                    match self.navigation.select(args) {
                        Ok(msg) => msg,
                        Err(e) => format!("Error: {}", e),
                    }
                }
            }
            "filter" => {
                if args.is_empty() {
                    "Usage: filter <expr> (e.g., filter role=UBO)".to_string()
                } else {
                    match self.navigation.filter(&args.join(" ")) {
                        Ok(msg) => msg,
                        Err(e) => format!("Error: {}", e),
                    }
                }
            }
            "clear-filter" => self.navigation.clear_filter(),
            
            // View commands
            "view" => {
                if args.is_empty() {
                    self.view.view()
                } else {
                    match self.view.view_set(args[0]) {
                        Ok(msg) => msg,
                        Err(e) => format!("Error: {}", e),
                    }
                }
            }
            "zoom" => {
                if args.is_empty() {
                    "Usage: zoom <level> (e.g., zoom 2.0)".to_string()
                } else {
                    match self.view.zoom(args[0]) {
                        Ok(msg) => msg,
                        Err(e) => format!("Error: {}", e),
                    }
                }
            }
            "center" => {
                match self.view.center(args.first().copied()) {
                    Ok(msg) => msg,
                    Err(e) => format!("Error: {}", e),
                }
            }
            
            // Help
            "help" | "?" => HELP_TEXT.to_string(),
            
            _ => format!("Unknown command: {}. Type 'help' for available commands.", command),
        }
    }
}

const HELP_TEXT: &str = r#"
Available Commands:

NAVIGATION:
  cd <path>           Navigate to scope (supports: .., relative, absolute)
  pwd                 Print current scope with mass
  ls [-t type]        List entities (optionally filter by type)
  focus <id>          Set focal entity
  select <ids...>     Multi-select entities
  filter <expr>       Apply filter (e.g., filter role=UBO)
  clear-filter        Remove all filters

VIEW CONTROL:
  view                Show current view mode
  view <mode>         Force view mode (astro, pyramid, matrix, hybrid, auto)
  zoom <level>        Set zoom level (0.1 - 10.0)
  center [id]         Center viewport on entity

HELP:
  help, ?             Show this help

EXAMPLES:
  cd allianz.trading.germany
  cd ..
  ls -t CBU
  focus 12345
  filter role=UBO
  view pyramid
  zoom 1.5
"#;
```

## Integration with REPL

Update REPL state to use CommandRouter:

```rust
pub struct ReplState {
    session: Arc<SessionManager>,
    command_router: CommandRouter,
    history: Vec<String>,
    prompt: String,
}

impl ReplState {
    pub fn new(
        session: Arc<SessionManager>,
        graph_loader: Arc<dyn GraphLoader>,
        view_controller: Arc<RwLock<ViewModeController>>,
    ) -> Self {
        let command_router = CommandRouter::new(
            session.clone(),
            graph_loader,
            view_controller,
        );
        
        Self {
            session,
            command_router,
            history: vec![],
            prompt: "> ".to_string(),
        }
    }
    
    pub async fn execute(&mut self, input: &str) -> String {
        self.history.push(input.to_string());
        self.command_router.execute(input).await
    }
    
    pub fn render_prompt(&self) -> String {
        let ctx = self.session.current();
        format!(
            "[{}] ({}) {}",
            ctx.scope.display(),
            ctx.mass.total,
            self.prompt
        )
    }
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cd_relative() {
        let (session, loader) = create_test_session();
        let nav = NavigationCommands::new(session.clone(), loader);
        
        // Start at root
        assert_eq!(session.current().scope.depth(), 0);
        
        // cd into child
        nav.cd("trading").await.unwrap();
        assert_eq!(session.current().scope.depth(), 1);
        
        // cd up
        nav.cd("..").await.unwrap();
        assert_eq!(session.current().scope.depth(), 0);
    }
    
    #[test]
    fn test_filter_parsing() {
        let filter = parse_filter_expr("role=UBO").unwrap();
        assert!(matches!(filter, Filter::ByRole(_)));
        
        let filter = parse_filter_expr("type=CBU").unwrap();
        assert!(matches!(filter, Filter::ByEntityType(_)));
    }
    
    #[test]
    fn test_pwd_output() {
        let session = create_test_session_with_scope("allianz.trading");
        let nav = NavigationCommands::new(session.clone(), mock_loader());
        
        let output = nav.pwd();
        assert!(output.contains("allianz.trading"));
        assert!(output.contains("SCOPE:"));
        assert!(output.contains("MASS:"));
    }
}
```

## Acceptance Criteria

- [ ] cd supports absolute and relative paths
- [ ] cd .. navigates up correctly
- [ ] pwd shows scope, mass, and entity counts
- [ ] ls groups entities by type
- [ ] ls -t filters by type
- [ ] focus updates viewport immediately
- [ ] filter applies and shows matching count
- [ ] view shows current mode with explanation
- [ ] view <mode> forces mode override
- [ ] view auto returns to mass-based selection
- [ ] help shows all commands

use zed_extension_api::{self as zed, LanguageServerId, Result};

struct DslExtension;

impl zed::Extension for DslExtension {
    fn new() -> Self {
        DslExtension
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        // Only handle our language server
        if language_server_id.as_ref() != "dsl-lsp" {
            return Err(format!(
                "Unknown language server: {}",
                language_server_id.as_ref()
            ));
        }

        // Try to find dsl-lsp in PATH first, then fall back to known location
        let path = worktree.which("dsl-lsp").unwrap_or_else(|| {
            // Absolute path to the built LSP binary
            "/Users/adamtc007/Developer/ob-poc/rust/target/release/dsl-lsp".to_string()
        });

        // Set up environment with config directory and EntityGateway URL
        let mut env = worktree.shell_env();
        env.push((
            "DSL_CONFIG_DIR".to_string(),
            "/Users/adamtc007/Developer/ob-poc/rust/config".to_string(),
        ));
        env.push((
            "ENTITY_GATEWAY_URL".to_string(),
            "http://[::1]:50051".to_string(),
        ));

        Ok(zed::Command {
            command: path,
            args: vec![],
            env,
        })
    }
}

zed::register_extension!(DslExtension);

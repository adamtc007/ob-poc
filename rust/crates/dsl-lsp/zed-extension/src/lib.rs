use zed_extension_api::{self as zed, LanguageServerId, Result};

struct DslExtension;

impl zed::Extension for DslExtension {
    fn new() -> Self {
        DslExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        // Try to find dsl-lsp in PATH
        let path = worktree
            .which("dsl-lsp")
            .unwrap_or_else(|| "dsl-lsp".to_string());

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(DslExtension);

use anyhow::Result;
use std::path::{Path, PathBuf};

const ZSH_INTEGRATION_SCRIPT: &str = r#"
__cli_app_osc() {
    builtin printf "\033]%s\007" "$1"
}

__cli_app_precmd() {
    local __exit=$?
    if [[ -n "$__cli_app_cmd_running" ]]; then
        __cli_app_osc "133;D;$__exit"
        unset __cli_app_cmd_running
    fi
    __cli_app_osc "7;file://${HOST}${PWD}"
    __cli_app_osc "133;A"
}

__cli_app_preexec() {
    __cli_app_osc "133;C"
    __cli_app_cmd_running=1
}

precmd_functions=(__cli_app_precmd ${precmd_functions[@]})
preexec_functions=(__cli_app_preexec ${preexec_functions[@]})
"#;

/// Manages temporary shell integration files that inject OSC 133 + OSC 7 hooks.
/// For zsh, uses the ZDOTDIR trick to source our integration after the user's config.
/// The temp directory is cleaned up when this struct is dropped.
pub struct ShellIntegration {
    temp_dir: PathBuf,
}

impl ShellIntegration {
    pub fn setup_zsh() -> Result<Self> {
        let temp_dir =
            std::env::temp_dir().join(format!("cli-app-shell-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir)?;

        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        let original_zdotdir = std::env::var("ZDOTDIR").unwrap_or_else(|_| home.clone());

        write_proxy_file(
            &temp_dir,
            ".zshenv",
            &original_zdotdir,
            ".zshenv",
            None,
        )?;

        write_proxy_file(
            &temp_dir,
            ".zprofile",
            &original_zdotdir,
            ".zprofile",
            None,
        )?;

        write_proxy_file(
            &temp_dir,
            ".zshrc",
            &original_zdotdir,
            ".zshrc",
            Some(ZSH_INTEGRATION_SCRIPT),
        )?;

        write_proxy_file(
            &temp_dir,
            ".zlogin",
            &original_zdotdir,
            ".zlogin",
            None,
        )?;

        Ok(Self { temp_dir })
    }

    pub fn zdotdir(&self) -> &Path {
        &self.temp_dir
    }
}

impl Drop for ShellIntegration {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

/// Writes a proxy dotfile that sources the user's original, then optionally appends extra script.
fn write_proxy_file(
    dest_dir: &Path,
    filename: &str,
    original_zdotdir: &str,
    original_filename: &str,
    append_script: Option<&str>,
) -> Result<()> {
    let original_path = format!("{}/{}", original_zdotdir, original_filename);
    let mut content = format!(
        r#"[[ -f "{path}" ]] && builtin source "{path}"
"#,
        path = original_path
    );

    if let Some(script) = append_script {
        content.push_str(script);
        // Restore ZDOTDIR so sub-shells use the user's original config
        content.push_str(&format!(
            r#"
ZDOTDIR="{orig}"
"#,
            orig = original_zdotdir
        ));
    }

    std::fs::write(dest_dir.join(filename), content)?;
    Ok(())
}

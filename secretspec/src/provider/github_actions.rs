use crate::provider::Provider;
use crate::{Result, SecretSpecError};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::process::Command;
use url::Url;

/// Configuration for the GitHub Actions provider.
///
/// This struct contains all the necessary configuration options for
/// interacting with GitHub Actions secrets via the GitHub CLI (`gh`).
///
/// # Write-Only Provider
///
/// **Important**: This provider is write-only because GitHub Actions secrets
/// are encrypted and cannot be read back via the CLI. The `gh secret list`
/// command only returns secret names, not their values.
///
/// # Examples
///
/// ```ignore
/// # use secretspec::provider::github_actions::GitHubActionsConfig;
/// // Using a repository
/// let config = GitHubActionsConfig {
///     repo: "owner/repo".to_string(),
///     account: None,
/// };
///
/// // With a specific GitHub account
/// let config = GitHubActionsConfig {
///     repo: "owner/repo".to_string(),
///     account: Some("myaccount".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitHubActionsConfig {
    /// The GitHub repository in OWNER/REPO format.
    ///
    /// This specifies the target repository where secrets will be stored.
    /// The repository acts as the namespace for secrets.
    pub repo: String,
    /// Optional account shorthand (for multiple GitHub accounts).
    ///
    /// Used with the `--hostname` flag when you have multiple GitHub
    /// accounts configured. This should match the hostname configured in
    /// `gh auth login`.
    pub account: Option<String>,
}

impl TryFrom<&Url> for GitHubActionsConfig {
    type Error = SecretSpecError;

    fn try_from(url: &Url) -> std::result::Result<Self, Self::Error> {
        let scheme = url.scheme();

        // Validate scheme
        match scheme {
            "ghactions" | "github-actions" => {}
            _ => {
                return Err(SecretSpecError::ProviderOperationFailed(format!(
                    "Invalid scheme '{}' for GitHub Actions provider. Use 'ghactions' or 'github-actions'",
                    scheme
                )));
            }
        }

        let mut config = Self::default();

        // Parse URL for repo and optional account
        // Format: ghactions://[account@]owner/repo
        if let Some(host) = url.host_str() {
            if host != "localhost" {
                // Check if we have username (account) information
                if !url.username().is_empty() {
                    config.account = Some(url.username().to_string());
                }

                // Combine host and path to form owner/repo
                let path = url.path().trim_start_matches('/');
                if path.is_empty() {
                    config.repo = host.to_string();
                } else {
                    config.repo = format!("{}/{}", host, path);
                }
            }
        }

        // Validate that we have a repository
        if config.repo.is_empty() {
            return Err(SecretSpecError::ProviderOperationFailed(
                "GitHub repository must be specified in format: ghactions://owner/repo".to_string(),
            ));
        }

        Ok(config)
    }
}

impl TryFrom<Url> for GitHubActionsConfig {
    type Error = SecretSpecError;

    fn try_from(url: Url) -> std::result::Result<Self, Self::Error> {
        (&url).try_into()
    }
}

/// Provider implementation for GitHub Actions secrets.
///
/// This provider integrates with GitHub CLI (`gh`) to store secrets for
/// GitHub Actions workflows. It is a **write-only** provider because GitHub
/// Actions secrets are encrypted and cannot be read back via the CLI.
///
/// # Write-Only Nature
///
/// The `get()` method always returns `Ok(None)` because GitHub does not
/// provide a way to retrieve the decrypted value of secrets through the CLI.
/// This is a security feature of GitHub Actions.
///
/// # Storage Structure
///
/// - **Project name**: Ignored (repository provides namespace)
/// - **Profile "default"**: Repository-level secrets
/// - **Profile name**: Environment-level secrets (environment = profile name)
/// - **Secret key**: Used directly as the GitHub Actions secret name
///
/// # Authentication
///
/// Requires authentication via `gh auth login`. The provider will check
/// authentication status before performing operations.
///
/// # Example Usage
///
/// ```ignore
/// # Interactive auth
/// gh auth login
/// secretspec set API_KEY --provider ghactions://owner/repo
///
/// # Environment-specific secret
/// secretspec set DATABASE_URL --profile production --provider ghactions://owner/repo
/// ```
pub struct GitHubActionsProvider {
    /// Configuration for the provider including repository and account.
    config: GitHubActionsConfig,
}

crate::register_provider! {
    struct: GitHubActionsProvider,
    config: GitHubActionsConfig,
    name: "github-actions",
    description: "GitHub Actions secrets (write-only)",
    schemes: ["ghactions", "github-actions"],
    examples: ["ghactions://owner/repo", "ghactions://account@owner/repo"],
}

impl GitHubActionsProvider {
    /// Creates a new GitHubActionsProvider with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration for the provider
    pub fn new(config: GitHubActionsConfig) -> Self {
        Self { config }
    }

    /// Executes a GitHub CLI command with proper error handling.
    ///
    /// This method handles:
    /// - Setting up authentication (account/hostname)
    /// - Executing the command
    /// - Parsing error messages for common issues
    /// - Providing helpful error messages for missing CLI
    ///
    /// # Arguments
    ///
    /// * `args` - The command arguments to pass to `gh`
    ///
    /// # Returns
    ///
    /// * `Result<String>` - The command output or an error
    ///
    /// # Errors
    ///
    /// Returns specific errors for:
    /// - Missing GitHub CLI installation
    /// - Authentication required
    /// - Command execution failures
    fn execute_gh_command(&self, args: &[&str]) -> Result<String> {
        let mut cmd = Command::new("gh");

        // Add hostname/account if specified
        if let Some(account) = &self.config.account {
            cmd.arg("--hostname").arg(account);
        }

        cmd.args(args);

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "GitHub CLI (gh) is not installed.\n\nTo install it:\n  - macOS: brew install gh\n  - Linux: See https://github.com/cli/cli/blob/trunk/docs/install_linux.md\n  - Windows: See https://github.com/cli/cli/blob/trunk/docs/install_windows.md\n\nAfter installation, run 'gh auth login' to authenticate.".to_string(),
                ));
            }
            Err(e) => return Err(e.into()),
        };

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            if error_msg.contains("not logged") || error_msg.contains("authentication") {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "GitHub authentication required. Please run 'gh auth login' first.".to_string(),
                ));
            }
            return Err(SecretSpecError::ProviderOperationFailed(
                error_msg.to_string(),
            ));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| SecretSpecError::ProviderOperationFailed(e.to_string()))
    }

    /// Checks if the user is authenticated with GitHub.
    ///
    /// Uses the `gh auth status` command to verify authentication.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - User is authenticated
    /// * `Ok(false)` - User is not authenticated
    /// * `Err(_)` - Command execution failed
    fn is_authenticated(&self) -> Result<bool> {
        match self.execute_gh_command(&["auth", "status"]) {
            Ok(_) => Ok(true),
            Err(SecretSpecError::ProviderOperationFailed(msg))
                if msg.contains("not logged") || msg.contains("authentication") =>
            {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }
}

impl Provider for GitHubActionsProvider {
    fn name(&self) -> &'static str {
        Self::PROVIDER_NAME
    }

    /// Checks if a secret exists in GitHub Actions.
    ///
    /// **Note**: This method cannot return the actual secret value because GitHub Actions
    /// secrets are encrypted and cannot be read back via the CLI for security reasons.
    /// However, it can check if a secret with the given name exists by using `gh secret list`.
    ///
    /// If the secret exists, returns a placeholder value to indicate existence.
    /// If the secret does not exist, returns `None`.
    ///
    /// This allows the import command to properly detect existing secrets and avoid
    /// overwriting them.
    ///
    /// # Arguments
    ///
    /// * `project` - **Ignored** (repository provides namespace)
    /// * `key` - The secret key name to check
    /// * `profile` - The profile name (maps to environment name)
    ///
    /// # Returns
    ///
    /// * `Ok(Some(_))` - Secret exists (value is a placeholder, not the real value)
    /// * `Ok(None)` - Secret does not exist
    /// * `Err(_)` - Error checking secret existence
    fn get(&self, _project: &str, key: &str, profile: &str) -> Result<Option<SecretString>> {
        // Check authentication status first
        if !self.is_authenticated()? {
            return Err(SecretSpecError::ProviderOperationFailed(
                "GitHub authentication required. Please run 'gh auth login' first.".to_string(),
            ));
        }

        // Build command to list secrets
        let mut args = vec!["secret", "list", "--repo", &self.config.repo];

        // Add --env flag for non-default profiles
        let env_flag;
        if profile != "default" {
            env_flag = format!("--env={}", profile);
            args.push(&env_flag);
        }

        // Execute the list command
        let output = match self.execute_gh_command(&args) {
            Ok(output) => output,
            Err(e) => {
                // If the environment doesn't exist, gh secret list will fail
                // In this case, the secret definitely doesn't exist
                return Ok(None);
            }
        };

        // Parse the output to check if our secret name exists
        // Output format is typically: NAME  UPDATED
        for line in output.lines() {
            // Skip header line
            if line.starts_with("NAME") || line.is_empty() {
                continue;
            }

            // Split on whitespace and check if first column matches our key
            if let Some(secret_name) = line.split_whitespace().next() {
                if secret_name == key {
                    // Secret exists! Return a placeholder value
                    // The actual value cannot be retrieved for security reasons
                    return Ok(Some(SecretString::new("***ENCRYPTED***".into())));
                }
            }
        }

        // Secret not found in the list
        Ok(None)
    }

    /// Stores a secret in GitHub Actions.
    ///
    /// Creates or updates a secret for GitHub Actions. Secrets can be stored
    /// at the repository level (default profile) or environment level (named profiles).
    ///
    /// # Arguments
    ///
    /// * `project` - **Ignored** (repository provides namespace)
    /// * `key` - The secret key name (used directly as GitHub Actions secret name)
    /// * `value` - The secret value to store
    /// * `profile` - The profile name:
    ///   - "default": Repository-level secret
    ///   - Other: Environment-level secret (environment name = profile name)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Secret stored successfully
    /// * `Err(_)` - Storage or authentication error
    ///
    /// # Errors
    ///
    /// - Authentication required if not signed in
    /// - Secret creation/update failures
    /// - Repository access issues
    fn set(&self, _project: &str, key: &str, value: &SecretString, profile: &str) -> Result<()> {
        // Check authentication status first
        if !self.is_authenticated()? {
            return Err(SecretSpecError::ProviderOperationFailed(
                "GitHub authentication required. Please run 'gh auth login' first.".to_string(),
            ));
        }

        let mut args = vec!["secret", "set", key, "--repo", &self.config.repo];

        // Add --env flag for non-default profiles
        let env_flag;
        if profile != "default" {
            env_flag = format!("--env={}", profile);
            args.push(&env_flag);
        }

        // gh secret set reads from stdin, so we need to pass the value via stdin
        let mut cmd = Command::new("gh");

        // Add hostname if specified
        if let Some(account) = &self.config.account {
            cmd.arg("--hostname").arg(account);
        }

        cmd.args(&args);
        cmd.arg("--body").arg(value.expose_secret());

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "GitHub CLI (gh) is not installed. Please install it first.".to_string(),
                ));
            }
            Err(e) => return Err(e.into()),
        };

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Failed to set secret '{}': {}",
                key, error_msg
            )));
        }

        Ok(())
    }
}

impl Default for GitHubActionsProvider {
    /// Creates a GitHubActionsProvider with default configuration.
    ///
    /// Note: This will have an empty repository string and should not be
    /// used directly. Use `new()` with a proper configuration instead.
    fn default() -> Self {
        Self::new(GitHubActionsConfig::default())
    }
}

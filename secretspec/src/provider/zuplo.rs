use crate::provider::Provider;
use crate::{Result, SecretSpecError};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::process::Command;
use url::Url;

/// Configuration for the Zuplo provider.
///
/// This struct contains all the necessary configuration options for
/// interacting with Zuplo variables via the Zuplo CLI.
///
/// # Write-Only Provider (No Read Support)
///
/// **Important**: This provider is write-only because the Zuplo CLI does not
/// provide a command to list or read variable values. The `get()` method will
/// always return an error.
///
/// This means:
/// - ✅ Works with `secretspec export` command
/// - ❌ Does NOT work with `secretspec import` command
/// - ❌ Cannot check if variables already exist
///
/// # Examples
///
/// ```ignore
/// # use secretspec::provider::zuplo::ZuploConfig;
/// // Using current project/branch context
/// let config = ZuploConfig::default();
///
/// // With a specific project
/// let config = ZuploConfig {
///     project: Some("my-api".to_string()),
///     branch: None,
/// };
///
/// // With both project and branch
/// let config = ZuploConfig {
///     project: Some("my-api".to_string()),
///     branch: Some("main".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ZuploConfig {
    /// The Zuplo project name.
    ///
    /// If not set, the Zuplo CLI will use the current project context.
    pub project: Option<String>,
    /// The branch name for environment-specific variables.
    ///
    /// If not set, the Zuplo CLI may apply variables to all environments
    /// or use the current branch context.
    pub branch: Option<String>,
}

impl TryFrom<&Url> for ZuploConfig {
    type Error = SecretSpecError;

    fn try_from(url: &Url) -> std::result::Result<Self, Self::Error> {
        let scheme = url.scheme();

        // Validate scheme
        if scheme != "zuplo" {
            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Invalid scheme '{}' for Zuplo provider. Use 'zuplo'",
                scheme
            )));
        }

        let mut config = Self::default();

        // Parse URL for optional project and branch
        // Format: zuplo://[project]/[branch]
        // Examples:
        //   zuplo:// -> no project, no branch
        //   zuplo://my-project -> project only
        //   zuplo://my-project/main -> project and branch
        if let Some(host) = url.host_str() {
            if !host.is_empty() && host != "localhost" {
                config.project = Some(host.to_string());

                // Check for branch in path
                let path = url.path().trim_start_matches('/');
                if !path.is_empty() {
                    config.branch = Some(path.to_string());
                }
            }
        }

        Ok(config)
    }
}

impl TryFrom<Url> for ZuploConfig {
    type Error = SecretSpecError;

    fn try_from(url: Url) -> std::result::Result<Self, Self::Error> {
        (&url).try_into()
    }
}

/// Provider implementation for Zuplo variable management.
///
/// This provider integrates with Zuplo CLI to store variables for
/// Zuplo API gateway projects. It is a **write-only** provider because
/// the Zuplo CLI does not provide a way to read variable values.
///
/// # Write-Only Nature (No Read Support)
///
/// Unlike GitHub Actions which can at least check if secrets exist, the
/// Zuplo provider has no read capability at all:
///
/// - The `get()` method always returns an error
/// - Cannot check if a variable already exists
/// - Not compatible with the `import` command
/// - Only works with the `export` command
///
/// # Storage Structure
///
/// - **Project name**: Optional, specified in URI or uses CLI context
/// - **Branch**: Maps to Zuplo environment
///   - Profile "default": No branch specified (may apply to all environments)
///   - Profile name: Used as `--branch` parameter
/// - **Variable name**: Used directly as the Zuplo variable name
/// - **Secret flag**: Always set to `--is-secret true`
///
/// # Authentication
///
/// Requires authentication via `zuplo login`. The provider will check
/// if the CLI is installed and provide helpful error messages.
///
/// # Example Usage
///
/// ```ignore
/// # Authenticate first
/// zuplo login
///
/// # Export secrets to Zuplo (from default provider like 1Password)
/// secretspec export zuplo://my-api/main
///
/// # With profile (exports to specific branch)
/// secretspec export zuplo://my-api/production --profile production
/// ```
pub struct ZuploProvider {
    /// Configuration for the provider including project and branch.
    config: ZuploConfig,
}

crate::register_provider! {
    struct: ZuploProvider,
    config: ZuploConfig,
    name: "zuplo",
    description: "Zuplo API gateway variables (write-only, export only)",
    schemes: ["zuplo"],
    examples: ["zuplo://", "zuplo://my-project", "zuplo://my-project/main"],
}

impl ZuploProvider {
    /// Creates a new ZuploProvider with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration for the provider
    pub fn new(config: ZuploConfig) -> Self {
        Self { config }
    }

    /// Executes a Zuplo CLI command with proper error handling.
    ///
    /// This method handles:
    /// - Executing the command
    /// - Parsing error messages for common issues
    /// - Providing helpful error messages for missing CLI
    ///
    /// # Arguments
    ///
    /// * `args` - The command arguments to pass to `zuplo`
    ///
    /// # Returns
    ///
    /// * `Result<String>` - The command output or an error
    ///
    /// # Errors
    ///
    /// Returns specific errors for:
    /// - Missing Zuplo CLI installation
    /// - Authentication required
    /// - Command execution failures
    fn execute_zuplo_command(&self, args: &[&str]) -> Result<String> {
        let mut cmd = Command::new("zuplo");
        cmd.args(args);

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "Zuplo CLI is not installed.\n\nTo install it:\n  npm install -g zuplo\n\nAfter installation, run 'zuplo login' to authenticate.".to_string(),
                ));
            }
            Err(e) => return Err(e.into()),
        };

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            if error_msg.contains("not logged")
                || error_msg.contains("authentication")
                || error_msg.contains("login")
            {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "Zuplo authentication required. Please run 'zuplo login' first.".to_string(),
                ));
            }
            return Err(SecretSpecError::ProviderOperationFailed(
                error_msg.to_string(),
            ));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| SecretSpecError::ProviderOperationFailed(e.to_string()))
    }

    /// Builds the command arguments for variable operations.
    ///
    /// Adds optional --project and --branch flags based on configuration
    /// and profile.
    ///
    /// # Arguments
    ///
    /// * `base_args` - The base command arguments (e.g., ["variable", "create"])
    /// * `profile` - The profile name for branch mapping
    ///
    /// # Returns
    ///
    /// A vector of argument strings including optional project/branch flags
    fn build_args(&self, base_args: Vec<String>, profile: &str) -> Vec<String> {
        let mut args = base_args;

        // Add project if specified
        if let Some(project) = &self.config.project {
            args.push("--project".to_string());
            args.push(project.clone());
        }

        // Add branch if specified in config or derived from profile
        // Profile "default" -> use config branch or none
        // Named profile -> use profile name as branch (overrides config)
        if profile != "default" {
            args.push("--branch".to_string());
            args.push(profile.to_string());
        } else if let Some(branch) = &self.config.branch {
            args.push("--branch".to_string());
            args.push(branch.clone());
        }

        args
    }
}

impl Provider for ZuploProvider {
    fn name(&self) -> &'static str {
        Self::PROVIDER_NAME
    }

    /// Attempts to read a variable from Zuplo.
    ///
    /// **Note**: This method always returns an error because the Zuplo CLI
    /// does not provide a command to read variable values. This is not a
    /// security feature like GitHub Actions, but rather a limitation of the
    /// CLI.
    ///
    /// This means:
    /// - This provider cannot be used with `secretspec import`
    /// - Only use with `secretspec export` command
    /// - Cannot check if variables already exist before writing
    ///
    /// # Arguments
    ///
    /// * `project` - **Ignored** (Zuplo CLI doesn't support reading)
    /// * `key` - The variable key name
    /// * `profile` - The profile name
    ///
    /// # Returns
    ///
    /// Always returns an error explaining that reading is not supported.
    ///
    /// # Errors
    ///
    /// Always returns `SecretSpecError::ProviderOperationFailed` with explanation
    fn get(&self, _project: &str, _key: &str, _profile: &str) -> Result<Option<SecretString>> {
        Err(SecretSpecError::ProviderOperationFailed(
            "Zuplo provider does not support reading variables.\n\nThe Zuplo CLI does not provide a command to list or read variable values.\nThis provider only works with the 'export' command, not 'import'.\n\nUse: secretspec export zuplo://your-project/branch".to_string(),
        ))
    }

    /// Stores a variable in Zuplo.
    ///
    /// Creates or updates a variable for Zuplo. The method attempts to create
    /// the variable first, and if it already exists, it will attempt to update it.
    ///
    /// # Arguments
    ///
    /// * `project` - **Ignored** (project specified in URI/config)
    /// * `key` - The variable name
    /// * `value` - The variable value to store
    /// * `profile` - The profile name (maps to branch):
    ///   - "default": Uses branch from config or none
    ///   - Other: Uses profile name as branch
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Variable stored successfully
    /// * `Err(_)` - Storage or authentication error
    ///
    /// # Errors
    ///
    /// - Authentication required if not logged in
    /// - Variable creation/update failures
    /// - Project access issues
    fn set(&self, _project: &str, key: &str, value: &SecretString, profile: &str) -> Result<()> {
        // Try to create the variable first
        let create_args = self.build_args(
            vec![
                "variable".to_string(),
                "create".to_string(),
                "--name".to_string(),
                key.to_string(),
                "--value".to_string(),
                value.expose_secret().to_string(),
                "--is-secret".to_string(),
                "true".to_string(),
            ],
            profile,
        );

        let create_args_str: Vec<&str> = create_args.iter().map(|s| s.as_str()).collect();

        match self.execute_zuplo_command(&create_args_str) {
            Ok(_) => return Ok(()),
            Err(SecretSpecError::ProviderOperationFailed(msg))
                if msg.contains("already exists") || msg.contains("duplicate") =>
            {
                // Variable exists, try to update it
                let update_args = self.build_args(
                    vec![
                        "variable".to_string(),
                        "update".to_string(),
                        "--name".to_string(),
                        key.to_string(),
                        "--value".to_string(),
                        value.expose_secret().to_string(),
                    ],
                    profile,
                );

                let update_args_str: Vec<&str> = update_args.iter().map(|s| s.as_str()).collect();
                self.execute_zuplo_command(&update_args_str)?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

impl Default for ZuploProvider {
    /// Creates a ZuploProvider with default configuration.
    ///
    /// Uses the current project/branch context from the Zuplo CLI.
    fn default() -> Self {
        Self::new(ZuploConfig::default())
    }
}

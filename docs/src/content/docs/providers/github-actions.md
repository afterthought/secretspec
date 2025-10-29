---
title: GitHub Actions Provider
description: GitHub Actions secrets management integration (write-only)
---

The GitHub Actions provider integrates with GitHub Actions for storing secrets used in CI/CD workflows. This is a **write-only** provider as GitHub Actions secrets are encrypted and cannot be read back via the CLI for security reasons.

## Prerequisites

- GitHub CLI (`gh`)
- GitHub account with access to the target repository
- Authenticated via `gh auth login`

## Configuration

### URI Format

```
ghactions://OWNER/REPO
github-actions://OWNER/REPO
ghactions://account@OWNER/REPO
```

- `OWNER`: GitHub organization or user name
- `REPO`: Repository name
- `account`: Optional GitHub hostname for multiple accounts

### Examples

```bash
# Use specific repository
$ secretspec set API_KEY --provider ghactions://myorg/myrepo

# Use specific GitHub account
$ secretspec set DATABASE_URL --provider ghactions://myaccount@myorg/myrepo

# Environment-specific secret (production profile → production environment)
$ secretspec set SECRET --profile production --provider ghactions://myorg/myrepo
```

## Write-Only Nature

**Important**: This provider is write-only because GitHub Actions secrets are encrypted and cannot be retrieved via the GitHub CLI.

- ✅ **Write**: Secrets can be created and updated
- ⚠️ **Read**: Secret **values** cannot be read back, but existence can be checked
- ✅ **Encrypted**: All secrets are encrypted by GitHub
- ✅ **Import-safe**: The `import` command works correctly and won't overwrite existing secrets

### How Get/Read Works

When you call `secretspec get API_KEY`, the provider:
1. Calls `gh secret list` to check if the secret name exists
2. Returns a **placeholder value** (`***ENCRYPTED***`) if the secret exists
3. Returns "not found" if the secret doesn't exist

This means:
- You **cannot** retrieve the actual secret value (security feature)
- You **can** check if a secret exists
- The `import` command **respects** existing secrets and won't overwrite them

This is a security feature of GitHub Actions to protect sensitive credentials.

## Usage

### Basic Commands

```bash
# Set a repository-level secret (default profile)
$ secretspec set API_KEY --provider ghactions://myorg/myrepo
Enter value for API_KEY: ********
✓ Secret API_KEY saved to GitHub Actions

# Set an environment-specific secret
$ secretspec set DATABASE_URL --profile production --provider ghactions://myorg/myrepo
Enter value for DATABASE_URL: ********
✓ Secret DATABASE_URL saved to GitHub Actions (environment: production)

# Check if a secret exists (returns placeholder value if it exists)
$ secretspec get API_KEY --provider ghactions://myorg/myrepo
***ENCRYPTED***  # Secret exists but value cannot be retrieved
```

### Importing from Other Providers

The primary use case is to import secrets from other providers (like 1Password) into GitHub Actions:

```bash
# Set up secretspec.toml with your secrets
$ cat secretspec.toml
[project]
name = "my-app"

[profiles.default]
API_KEY = { required = true }
DATABASE_URL = { required = true }

# Import from 1Password to GitHub Actions (repository-level)
$ secretspec import onepassword://Development --provider ghactions://myorg/myrepo

# Import to a specific environment (production profile)
$ secretspec import onepassword://Production --provider ghactions://myorg/myrepo --profile production
```

This workflow allows you to:
1. Store secrets in 1Password (or another provider)
2. Import them to GitHub Actions for your CI/CD workflows
3. Existing secrets in GitHub Actions won't be overwritten

### Storage Mapping

The provider maps SecretSpec concepts to GitHub Actions as follows:

| SecretSpec | GitHub Actions |
|------------|----------------|
| Profile "default" | Repository-level secret |
| Profile name (e.g., "production") | Environment-level secret (environment: "production") |
| Secret key | GitHub Actions secret name (used directly) |
| Project name | **Ignored** (repository provides namespace) |

### Secret Naming

Unlike some other providers, the **project name is not used** in secret naming. The secret key is used directly as the GitHub Actions secret name:

```toml
[project]
name = "my-app"  # This is ignored for GitHub Actions

[profiles.default]
API_KEY = { required = true }
```

Running `secretspec set API_KEY` creates a secret named `API_KEY` (not `my-app_API_KEY`).

### Environment-Specific Secrets

Environments in GitHub Actions provide additional controls like required reviewers and deployment protection rules. When you use a non-default profile, secrets are stored in an environment with that name:

```bash
# Repository-level secret (default profile)
$ secretspec set API_KEY --provider ghactions://myorg/myrepo

# Environment-level secret (production environment)
$ secretspec set DB_PASSWORD --profile production --provider ghactions://myorg/myrepo
```

**Note**: If the environment doesn't exist, `gh secret set --env` will create it automatically.

## Authentication

### Initial Setup

```bash
# Authenticate with GitHub
$ gh auth login

# Verify authentication
$ gh auth status
```

### Multiple Accounts

If you have multiple GitHub accounts configured:

```bash
# Use specific hostname
$ secretspec set API_KEY --provider ghactions://enterprise@myorg/myrepo
```

## Limitations

1. **Write-Only Values**: Secret values cannot be read back (security feature), but existence can be checked
2. **No Reflection**: Cannot list existing secrets with their values
3. **Requires Repository Access**: Must have write access to the repository
4. **Project Name Ignored**: The project name from secretspec.toml is not used in secret naming

## Troubleshooting

### "GitHub CLI not found"

Install the GitHub CLI:
- **macOS**: `brew install gh`
- **Linux**: See [installation guide](https://github.com/cli/cli/blob/trunk/docs/install_linux.md)
- **Windows**: See [installation guide](https://github.com/cli/cli/blob/trunk/docs/install_windows.md)

### "Authentication required"

Run `gh auth login` to authenticate with GitHub.

### "Resource not accessible by integration"

Ensure you have write access to the repository and that the token has the `repo` scope:

```bash
$ gh auth refresh -s repo
```

## Best Practices

1. **Use Environments for Production**: Create GitHub environments for production secrets to leverage protection rules
2. **Separate Profiles**: Use different profiles for different environments
3. **Audit Trail**: GitHub maintains an audit log of secret changes
4. **Rotate Regularly**: Use `secretspec set` to rotate secrets periodically
5. **Principle of Least Privilege**: Only grant repository access to those who need to set secrets

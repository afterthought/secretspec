---
title: Zuplo Provider
description: Zuplo API gateway variable management (write-only, export command only)
---

The Zuplo provider integrates with Zuplo API gateway for managing environment variables and secrets. This is a **write-only** provider as the Zuplo CLI does not provide commands to read or list variable values.

## Prerequisites

- Zuplo CLI (installed via `npx` automatically, or globally via `npm install -g zuplo`)
- Zuplo account with access to your project
- Zuplo API key for deployment (no authentication needed for variable operations)

## Configuration

### URI Format

```
zuplo://
zuplo://PROJECT
zuplo://PROJECT/BRANCH
```

- `PROJECT`: Optional Zuplo project identifier
- `BRANCH`: Optional branch/environment name
- If omitted, the Zuplo CLI uses the current project context

### Examples

```bash
# Use current project/branch context
$ secretspec export zuplo://

# Specify project
$ secretspec export zuplo://my-api

# Specify both project and branch
$ secretspec export zuplo://my-api/main

# With profile (branch overrides profile name)
$ secretspec export zuplo://my-api/production --profile production
```

## Write-Only Nature (No Read Support)

**Critical**: This provider is write-only because the Zuplo CLI does not provide commands to read variable values.

- ❌ **Cannot read**: No way to retrieve existing values
- ❌ **Cannot check existence**: Cannot verify if variables already exist
- ❌ **Not compatible with `import`**: The `import` command requires read support
- ✅ **Works with `export`**: Use to push secrets from other providers to Zuplo
- ✅ **Works with `set`**: Directly set individual secrets in Zuplo
- ✅ **Works with `check`**: Only when checking your default provider (not Zuplo itself)

### Why This Matters

Unlike GitHub Actions (which can at least check if secrets exist), the Zuplo provider has no read capability at all. This means:

1. ✅ `secretspec set KEY --provider zuplo://...` - Works (sets individual secret)
2. ✅ `secretspec export zuplo://...` - Works (exports from default provider to Zuplo)
3. ✅ `secretspec check` - Works (checks default provider, not Zuplo)
4. ❌ `secretspec check --provider zuplo://...` - Fails (tries to read from Zuplo)
5. ❌ `secretspec import zuplo://...` - Fails (tries to read from Zuplo)
6. The provider will attempt to create variables, falling back to update if they already exist

## Primary Use Case: GitHub Actions Workflow

The primary use case for the Zuplo provider is to export secrets from your primary provider (like 1Password) to Zuplo as part of a deployment workflow.

### Typical GitHub Actions Workflow

```yaml
name: Deploy to Zuplo

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup development environment
        uses: cachix/devenv-action@v1

      - name: Check secrets exist in 1Password
        env:
          OP_SERVICE_ACCOUNT_TOKEN: ${{ secrets.OP_SERVICE_ACCOUNT_TOKEN }}
        run: secretspec check

      - name: Export secrets to Zuplo
        env:
          OP_SERVICE_ACCOUNT_TOKEN: ${{ secrets.OP_SERVICE_ACCOUNT_TOKEN }}
        run: secretspec export zuplo://my-api/main

      - name: Deploy to Zuplo
        env:
          ZUPLO_API_KEY: ${{ secrets.ZUPLO_API_KEY }}
        run: |
          npx zuplo deploy
```

### Workflow Steps Explained

1. **Setup**: Use devenv to configure the development environment (installs secretspec, op CLI, etc.)
2. **Check**: Verify all required secrets exist in 1Password (or your default provider)
3. **Export**: Push secrets from 1Password to Zuplo
4. **Deploy**: Run your Zuplo deployment with API key authentication

This ensures that:
- All secrets are validated in your source provider before deployment
- Secrets are fresh and up-to-date in Zuplo
- Your Zuplo API has access to all required credentials
- No need for interactive authentication in CI/CD

## Usage

### Export from Default Provider

```bash
# Export to Zuplo (reads from default provider)
$ secretspec export zuplo://my-api/main

# Export specific profile
$ secretspec export zuplo://my-api/production --profile production
```

### Configuration File Setup

```toml
# secretspec.toml
[project]
name = "my-api"

[profiles.default]
API_KEY = { required = true }
DATABASE_URL = { required = true }

[profiles.production]
# Production-specific secrets
STRIPE_SECRET_KEY = { required = true }
```

### Storage Mapping

The provider maps SecretSpec concepts to Zuplo as follows:

| SecretSpec | Zuplo CLI |
|------------|-----------|
| Profile "default" | No `--branch` flag (may apply to all environments) |
| Profile name (e.g., "production") | `--branch production` flag |
| Secret key | Variable name with `--is-secret true` |
| Project name from config | **Ignored** (project specified in URI) |
| Project from URI | `--project` flag |
| Branch from URI | `--branch` flag (overridden by profile if not "default") |

### Variable Naming

The secret key is used directly as the Zuplo variable name:

```toml
[project]
name = "my-api"  # Ignored for Zuplo

[profiles.default]
API_KEY = { required = true }
```

Running `secretspec export zuplo://my-api/main` creates a variable named `API_KEY` (not `my-api_API_KEY`).

## Authentication

The Zuplo CLI variable management commands do not require authentication. Authentication is only needed for deployment operations using the `--apiKey` flag.

### Local Development

```bash
# No authentication needed for variable operations
$ secretspec set API_KEY --provider zuplo://my-api/main
$ secretspec export zuplo://my-api/main
```

### CI/CD Authentication

For deployment in GitHub Actions or other CI/CD systems, use an API key:

```bash
# Set API key as environment variable
export ZUPLO_API_KEY="your-api-key-here"

# Use in deployment
npx zuplo deploy
```

## Error Handling

The provider implements intelligent error handling:

1. **First attempt**: Try to create the variable
2. **If exists**: Automatically update instead
3. **If fails**: Return helpful error message

This means you don't need to worry about whether variables already exist - the provider handles both cases automatically.

## Limitations

1. **Write-Only**: Cannot read variable values (not a security feature, just unavailable)
2. **No Import Support**: Cannot use with `secretspec import` command
3. **No Existence Check**: Cannot verify if variables exist before writing
4. **Requires Zuplo CLI**: Must have `zuplo` command installed and in PATH
5. **Project URI Required**: For explicit project targeting, must specify in URI

## Comparison with GitHub Actions Provider

| Feature | GitHub Actions | Zuplo |
|---------|----------------|-------|
| Write secrets | ✅ Yes | ✅ Yes |
| Read values | ❌ No (encrypted) | ❌ No (unavailable) |
| Check existence | ✅ Yes (via list) | ❌ No |
| Import command | ✅ Yes (checks existence) | ❌ No |
| Export command | ✅ Yes | ✅ Yes |
| Error handling | Manual check | Automatic create/update |

## Troubleshooting

### "Zuplo CLI is not installed"

The Zuplo CLI is installed automatically via `npx` when running `zuplo deploy`. For manual installation:
```bash
npm install -g zuplo
```

### "Provider does not support reading variables"

This is expected behavior. The Zuplo CLI does not provide commands to read variable values. Use the `export` command instead of `import`:

```bash
# ❌ This will not work
secretspec import zuplo://my-api

# ✅ Use this instead
secretspec export zuplo://my-api/main
```

### Variables not updating

The provider automatically attempts to update existing variables. If updates fail:
1. Verify the project and branch names are correct
2. Check that the Zuplo API allows variable updates
3. Ensure you have the correct permissions for the project

## Best Practices

1. **Use with Export Command**: Always use `export`, never `import`
2. **Verify Before Export**: Run `secretspec check` to ensure all secrets exist in your source provider
3. **Automate in CI/CD**: Integrate into deployment workflows to ensure fresh secrets
4. **Profile Mapping**: Use profiles to target different Zuplo branches/environments
5. **Source of Truth**: Keep 1Password (or your preferred provider) as the source of truth
6. **Deployment Order**: Always export secrets before deploying your Zuplo API

## Example: Complete Deployment Flow

```bash
# 1. Check that all secrets exist in your source provider (1Password)
$ secretspec check
✓ All required secrets found

# 2. Export secrets to Zuplo production environment
$ secretspec export zuplo://my-api/production --profile production
✓ Exported 15 secrets to Zuplo

# 3. Deploy your Zuplo API
$ zuplo deploy
✓ Deployment successful

# Your Zuplo API now has access to all required secrets
```

# Orbit-RS Scripts

Utility scripts for development, deployment, and maintenance.

## Available Scripts

### prepare-secrets.sh

Automated script for preparing Kubernetes secrets for GitHub Actions CI/CD pipeline.

**Purpose:** Generate base64-encoded kubeconfig files for staging and production deployments.

**Features:**
- Interactive kubectl context selection
- Service account creation with minimal RBAC permissions (recommended)
- Support for existing kubeconfig usage
- Automatic namespace creation
- Connection testing and validation
- Secure credential handling

**Usage:**

```bash
# For staging environment
./scripts/prepare-secrets.sh staging

# For production environment
./scripts/prepare-secrets.sh production
```

**Requirements:**
- `kubectl` installed and configured
- Access to target Kubernetes cluster
- `base64` command available (standard on macOS/Linux)

**Output:**
- Creates `{environment}-kubeconfig-base64.txt` file
- Base64-encoded kubeconfig ready for GitHub Secrets
- Provides step-by-step instructions for adding to GitHub

**Documentation:** See [docs/SECRETS_CONFIGURATION_GUIDE.md](../docs/SECRETS_CONFIGURATION_GUIDE.md) for complete guide.

## Adding New Scripts

When adding new scripts to this directory:

1. **Make executable:** `chmod +x scripts/your-script.sh`
2. **Add shebang:** Start with `#!/bin/bash` or appropriate interpreter
3. **Add error handling:** Use `set -e` for bash scripts
4. **Document:** Add entry to this README
5. **Test:** Verify script works in clean environment
6. **Version control:** Commit script with descriptive message

## Best Practices

- Use descriptive names: `verb-noun.sh` format
- Include usage examples in comments
- Validate prerequisites before execution
- Provide helpful error messages
- Clean up temporary files
- Use colors for output clarity (but gracefully degrade)
- Test on both macOS and Linux when possible

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for contribution guidelines.

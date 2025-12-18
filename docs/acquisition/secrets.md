# Secrets

Secrets reference environment variables for sensitive values like API keys and tokens.

## Schema

```yaml
secrets:
  secret_name: "${ENVIRONMENT_VARIABLE}"
```

## Example

```yaml
acquisition:
  secrets:
    api_key: "${THREAT_INTEL_API_KEY}"
    auth_token: "${AZURE_AUTH_TOKEN}"
    database_url: "${DB_CONNECTION_STRING}"
```

## Environment Variable Syntax

Use `${VAR_NAME}` to reference environment variables:

```yaml
secrets:
  my_secret: "${MY_ENV_VAR}"
```

The variable is resolved at execution time from the environment.

## Using Secrets in Steps

Reference secrets via `{{secrets.name}}`:

```yaml
steps:
  - name: api_call
    type: http
    request:
      url: "https://api.example.com/check"
      headers:
        Authorization: "Bearer {{secrets.api_key}}"
        X-API-Key: "{{secrets.another_key}}"
```

## Error Handling

If an environment variable is not set:

- Execution fails with an error message
- The error identifies which secret is missing
- No partial execution occurs

## Security Best Practices

### Don't Commit Secrets

Never put actual secret values in pack files:

```yaml
# WRONG - secret exposed in file
secrets:
  api_key: "sk-1234567890abcdef"

# CORRECT - reference environment variable
secrets:
  api_key: "${API_KEY}"
```

### Use .env Files Locally

For local development, use `.env` files (add to `.gitignore`):

```bash
# .env
THREAT_INTEL_API_KEY=your-key-here
AZURE_AUTH_TOKEN=your-token-here
```

### CI/CD Secrets

In CI/CD pipelines, use the platform's secret management:

- GitHub Actions: Repository secrets
- Azure DevOps: Variable groups with secrets
- GitLab: CI/CD variables

## Common Use Cases

### API Authentication

```yaml
secrets:
  abuseipdb_key: "${ABUSEIPDB_API_KEY}"
  virustotal_key: "${VIRUSTOTAL_API_KEY}"

steps:
  - name: check_ip
    type: http
    request:
      url: "https://api.abuseipdb.com/api/v2/check"
      headers:
        Key: "{{secrets.abuseipdb_key}}"
```

### Azure API Access

```yaml
secrets:
  azure_token: "${AZURE_ACCESS_TOKEN}"

steps:
  - name: graph_call
    type: http
    request:
      url: "https://graph.microsoft.com/v1.0/users"
      auth: azure  # Uses Azure CLI credential instead
```

Note: For Azure APIs, consider using `auth: azure` which uses the Azure CLI credential automatically.

## Related

- [HTTP Steps](steps/http.md) - Using secrets in API calls
- [Variable Syntax](../reference/variable-syntax.md) - How secrets are substituted

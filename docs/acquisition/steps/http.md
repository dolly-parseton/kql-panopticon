# HTTP Steps

HTTP steps call external APIs for data enrichment and correlation.

## Schema

```yaml
- name: step_name             # Required: unique identifier
  type: http                  # Required: must be "http"

  request:                    # Required: request configuration
    method: GET               # GET | POST | PUT | DELETE
    url: "https://..."        # URL (supports variables)
    params: {...}             # Query parameters
    headers: {...}            # Request headers
    body: {...}               # Request body (POST/PUT)
    auth: azure               # Optional: authentication

  response:                   # Required: response mapping
    fields:                   # JSONPath field extraction
      column: "$.path"

  rate_limit:                 # Optional: rate limiting
    requests: 10
    per: second

  depends_on: [...]           # Optional: dependencies
  when: "{{...}}"             # Optional: condition
  on_error: continue          # Optional: error handling
```

## Request Configuration

### Methods

| Method | Use Case |
|--------|----------|
| `GET` | Retrieve data |
| `POST` | Submit data |
| `PUT` | Update data |
| `DELETE` | Remove data |

### URL

Supports variable substitution:

```yaml
request:
  method: GET
  url: "https://api.example.com/users/{{inputs.user_id}}"
```

### Query Parameters

```yaml
request:
  method: GET
  url: "https://api.example.com/check"
  params:
    ip: "{{inputs.ip_address}}"
    verbose: "true"
# Result: https://api.example.com/check?ip=8.8.8.8&verbose=true
```

### Headers

```yaml
request:
  method: GET
  url: "https://api.example.com/data"
  headers:
    Authorization: "Bearer {{secrets.api_key}}"
    Accept: "application/json"
    X-Custom-Header: "value"
```

### Body

For POST/PUT requests:

```yaml
request:
  method: POST
  url: "https://api.example.com/analyze"
  headers:
    Content-Type: "application/json"
  body:
    ips: ["{{inputs.ip_address}}"]
    options:
      detailed: true
```

### Authentication

| Value | Description |
|-------|-------------|
| `azure` | Use Azure CLI credential |
| `none` | No authentication (default) |

```yaml
request:
  method: GET
  url: "https://management.azure.com/..."
  auth: azure
```

## Response Mapping

Extract fields from JSON responses using JSONPath:

```yaml
response:
  fields:
    ip_address: "$.data.ip"
    threat_score: "$.data.threat.score"
    categories: "$.data.threat.categories[*]"
```

### JSONPath Examples

| JSONPath | Description |
|----------|-------------|
| `$.field` | Top-level field |
| `$.nested.field` | Nested field |
| `$.array[0]` | First array element |
| `$.array[*]` | All array elements |
| `$.array[*].field` | Field from each element |

## For-Each Iteration

Execute one request per value from a previous step:

```yaml
- name: enrich_ips
  type: http
  depends_on: [suspicious_ips]
  request:
    url: "https://api.abuseipdb.com/api/v2/check"
    params:
      ipAddress: "{{suspicious_ips.IPAddress | for_each}}"
    headers:
      Key: "{{secrets.abuseipdb_key}}"
  response:
    fields:
      ip: "$.data.ipAddress"
      score: "$.data.abuseConfidenceScore"
```

**Behavior**:
- If `suspicious_ips` has 5 IPs, 5 requests are made
- Results are automatically appended
- Rate limiting applies across all iterations

**Restrictions**:
- Only one `for_each` per step
- Not available in KQL steps

## Rate Limiting

Prevent overwhelming external APIs:

```yaml
rate_limit:
  requests: 10
  per: second
```

| Period | Description |
|--------|-------------|
| `second` | Per second |
| `minute` | Per minute |
| `hour` | Per hour |

## Complete Example

```yaml
steps:
  - name: suspicious_ips
    type: kql
    query: |
      SigninLogs
      | where ResultType != 0
      | summarize FailedCount = count() by IPAddress
      | where FailedCount > 10
      | project IPAddress

  - name: threat_enrichment
    type: http
    depends_on: [suspicious_ips]
    when: "{{suspicious_ips | is_not_empty}}"

    request:
      method: GET
      url: "https://api.abuseipdb.com/api/v2/check"
      params:
        ipAddress: "{{suspicious_ips.IPAddress | for_each}}"
        maxAgeInDays: "90"
      headers:
        Key: "{{secrets.abuseipdb_key}}"
        Accept: "application/json"

    response:
      fields:
        ip: "$.data.ipAddress"
        score: "$.data.abuseConfidenceScore"
        country: "$.data.countryCode"
        isp: "$.data.isp"
        reports: "$.data.totalReports"

    rate_limit:
      requests: 30
      per: minute

    on_error: continue
```

## Using Results

HTTP step results are available like KQL results:

```yaml
# In conditions
when: "{{threat_enrichment | any(score > 80)}}"

# In KQL queries
query: |
  let malicious = dynamic([
    {{threat_enrichment | filter(score > 80).ip | array}}
  ]);
  SigninLogs | where IPAddress in (malicious)
```

## Error Handling

| Value | Behavior |
|-------|----------|
| `fail` | Stop on any request failure |
| `skip` | Skip entire step on failure |
| `continue` | Record failures, aggregate successful responses |

With `for_each`, `continue` allows partial success:

```yaml
- name: enrich
  type: http
  on_error: continue  # Some IPs may fail, others succeed
  request:
    params:
      ip: "{{ips.IPAddress | for_each}}"
```

## Related

- [Secrets](../secrets.md) - API key configuration
- [Step Options](../options.md) - Dependencies, conditions
- [Variable Syntax](../../reference/variable-syntax.md) - Substitution syntax
- [Transform Reference](../../reference/transforms.md) - for_each and other transforms

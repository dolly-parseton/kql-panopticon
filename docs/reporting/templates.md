# Tera Templates

Reports use the Tera templating engine for dynamic content generation.

## Basic Syntax

| Syntax | Purpose |
|--------|---------|
| `{{ variable }}` | Output variable value |
| `{% if %}...{% endif %}` | Conditional blocks |
| `{% for %}...{% endfor %}` | Loops |
| `{# comment #}` | Comments (not rendered) |

## Variables

### Accessing Values

```jinja2
{{ meta.timestamp }}
{{ inputs.target_user }}
{{ processing.risk_assessment.score }}
```

### Nested Access

```jinja2
{{ processing.risk_assessment.matched_indicators }}
{{ row.UserPrincipalName }}
```

## Available Variables

### meta

Execution metadata:

| Variable | Description |
|----------|-------------|
| `meta.timestamp` | RFC3339 timestamp |
| `meta.pack_name` | Pack name |
| `meta.workspace` | Workspace name |
| `meta.workspace_id` | Workspace ID |
| `meta.subscription` | Subscription name |
| `meta.subscription_id` | Subscription ID |

### inputs

User-provided values:

```jinja2
{{ inputs.target_user }}
{{ inputs.days }}
```

### Acquisition Steps

Access step results directly or via `acquisition`:

```jinja2
{# Direct access #}
{% for row in signins %}...{% endfor %}

{# Via acquisition namespace #}
{% for row in acquisition.signins %}...{% endfor %}
```

### Processing Results

```jinja2
{{ processing.risk_assessment.score }}
{{ processing.risk_assessment.level }}
{{ processing.risk_assessment.summary }}
{{ processing.risk_assessment.recommendation }}
{{ processing.risk_assessment.matched_indicators }}
```

## Conditionals

### Basic If

```jinja2
{% if processing.risk_assessment.score > 50 %}
**HIGH RISK**
{% endif %}
```

### If-Else

```jinja2
{% if processing.risk_assessment.level == "CRITICAL" %}
Immediate action required!
{% elif processing.risk_assessment.level == "HIGH" %}
Investigate within 24 hours.
{% else %}
Continue monitoring.
{% endif %}
```

### Truthy Checks

```jinja2
{% if processing.risk_assessment.summary %}
{{ processing.risk_assessment.summary }}
{% endif %}
```

## Loops

### Basic Loop

```jinja2
{% for row in signins %}
- {{ row.UserPrincipalName }}: {{ row.SigninCount }} sign-ins
{% endfor %}
```

### Loop with Index

```jinja2
{% for row in signins %}
{{ loop.index }}. {{ row.UserPrincipalName }}
{% endfor %}
```

### Loop Variables

| Variable | Description |
|----------|-------------|
| `loop.index` | 1-based index |
| `loop.index0` | 0-based index |
| `loop.first` | True if first iteration |
| `loop.last` | True if last iteration |

### Empty Check

```jinja2
{% if signins %}
{% for row in signins %}...{% endfor %}
{% else %}
No sign-in data found.
{% endif %}
```

## Filters

Filters transform values:

```jinja2
{{ value | filter_name }}
{{ value | filter(arg="value") }}
```

### Common Filters

| Filter | Description | Example |
|--------|-------------|---------|
| `length` | Array/string length | `{{ signins \| length }}` |
| `default` | Default if empty | `{{ value \| default(value="N/A") }}` |
| `upper` | Uppercase | `{{ level \| upper }}` |
| `lower` | Lowercase | `{{ name \| lower }}` |
| `first` | First element | `{{ items \| first }}` |
| `last` | Last element | `{{ items \| last }}` |
| `join` | Join array | `{{ items \| join(sep=", ") }}` |
| `round` | Round number | `{{ score \| round(precision=2) }}` |

### Date Filters

```jinja2
{{ meta.timestamp | date(format="%Y-%m-%d") }}
```

## Tables

### Markdown Tables

```jinja2
| User | Sign-ins | Failed |
|------|----------|--------|
{% for row in signins %}
| {{ row.UserPrincipalName }} | {{ row.SigninCount }} | {{ row.FailedCount }} |
{% endfor %}
```

### Dynamic Headers

```jinja2
| Metric | Value |
|--------|-------|
| Score | {{ processing.risk_assessment.score }} |
| Level | {{ processing.risk_assessment.level }} |
```

## Lists

### Unordered

```jinja2
{% for row in signins %}
- {{ row.UserPrincipalName }}: {{ row.SigninCount }}
{% endfor %}
```

### Ordered

```jinja2
{% for row in signins %}
{{ loop.index }}. {{ row.UserPrincipalName }}
{% endfor %}
```

### Nested

```jinja2
{% for ind in processing.risk_assessment.matched_indicators %}
- **{{ ind.name }}** (+{{ ind.weight }})
  - {{ ind.description }}
{% endfor %}
```

## Whitespace Control

Control whitespace around blocks:

```jinja2
{%- if condition -%}
No extra whitespace
{%- endif -%}
```

## Complete Example

```jinja2
# Security Assessment Report

**Generated:** {{ meta.timestamp }}
**Workspace:** {{ meta.workspace }}
**Pack:** {{ meta.pack_name }}

---

## Risk Assessment

| Metric | Value |
|--------|-------|
| Score | {{ processing.risk_assessment.score }} |
| Level | {{ processing.risk_assessment.level }} |

{% if processing.risk_assessment.summary %}
### Summary
{{ processing.risk_assessment.summary }}
{% endif %}

---

## Sign-in Activity

{% if signins %}
| User | Sign-ins | Failed | Failure Rate |
|------|----------|--------|--------------|
{% for row in signins %}
| {{ row.UserPrincipalName }} | {{ row.TotalSignins }} | {{ row.TotalFailed }} | {{ row.FailureRate }}% |
{% endfor %}
{% else %}
*No sign-in data available.*
{% endif %}

---

## Matched Risk Indicators

{% if processing.risk_assessment.matched_indicators %}
{% for ind in processing.risk_assessment.matched_indicators %}
- **{{ ind.name }}** (+{{ ind.weight }} points)
  - {{ ind.description | default(value="No description") }}
{% endfor %}
{% else %}
*No risk indicators matched.*
{% endif %}

---

## Recommendation

{% if processing.risk_assessment.recommendation %}
{{ processing.risk_assessment.recommendation }}
{% else %}
Continue standard monitoring procedures.
{% endif %}

---

*Report generated by KQL Panopticon*
```

## Tera Documentation

For complete Tera syntax reference, see the official documentation:
[Tera Template Engine](https://tera.netlify.app/docs/)

## Related

- [Reporting Overview](overview.md) - Report configuration
- [Scoring](../processing/scoring.md) - Scoring results in templates

# Reporting Phase

The reporting phase generates formatted output from acquisition and processing results.

## Schema

```yaml
reporting:
  reports:
    - name: report_name       # Required: unique identifier
      format: markdown        # Optional: markdown | html | json
      output: "file.md"       # Optional: output filename
      template: |             # Template content (inline)
        # Report
        ...
      template_file: "..."    # OR external template file
      when: "{{...}}"         # Optional: conditional generation
```

## Report Definition

```yaml
reports:
  - name: summary
    format: markdown
    output: "summary-{{meta.timestamp}}.md"
    template: |
      # Investigation Summary

      **Generated**: {{ meta.timestamp }}
      **Workspace**: {{ meta.workspace }}

      ## Findings
      {% for row in signins %}
      - {{ row.UserPrincipalName }}: {{ row.SigninCount }} sign-ins
      {% endfor %}
```

### Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `name` | Yes | - | Unique report identifier |
| `format` | No | `markdown` | Output format |
| `output` | No | `{name}.{ext}` | Output filename |
| `template` | * | - | Inline template content |
| `template_file` | * | - | Path to template file |
| `when` | No | - | Condition for generation |

\* Either `template` or `template_file` required.

## Output Formats

| Format | Extension | Description |
|--------|-----------|-------------|
| `markdown` | `.md` | Markdown text (default) |
| `html` | `.html` | HTML document |
| `json` | `.json` | JSON data export |

## Template Sources

### Inline Template

```yaml
- name: summary
  template: |
    # Report Title
    Content here...
```

### External Template File

```yaml
- name: detailed
  template_file: "templates/report.md"
```

Path is relative to the pack location.

## Conditional Reports

Generate reports only when conditions are met:

```yaml
reports:
  - name: critical_alert
    when: "{{processing.risk_assessment.level | first | eq('CRITICAL')}}"
    template: |
      # CRITICAL ALERT
      Immediate action required...

  - name: no_findings
    when: "{{signins | is_empty}}"
    template: |
      # No Findings
      No relevant data found.
```

## Multiple Reports

Define multiple reports in a single pack:

```yaml
reports:
  - name: executive-summary
    format: markdown
    output: "executive-summary.md"
    template: |
      # Executive Summary
      ...

  - name: technical-details
    format: markdown
    output: "technical-details.md"
    template: |
      # Technical Details
      ...

  - name: data-export
    format: json
    output: "data.json"
```

## Complete Example

```yaml
reporting:
  reports:
    - name: executive-summary
      format: markdown
      output: "executive-summary.md"
      template: |
        # Security Assessment Summary

        **Generated:** {{ meta.timestamp }}
        **Workspace:** {{ meta.workspace }}

        ## Risk Assessment

        | Metric | Value |
        |--------|-------|
        | Score | {{ processing.risk_assessment.score }} |
        | Level | {{ processing.risk_assessment.level }} |

        {{ processing.risk_assessment.summary }}

        ## Key Findings

        {% for row in signins %}
        - **{{ row.UserPrincipalName }}**: {{ row.SigninCount }} sign-ins
        {% endfor %}

        ## Recommendation

        {{ processing.risk_assessment.recommendation }}

    - name: high-risk-alert
      when: "{{processing.risk_assessment.score | gt(50)}}"
      format: markdown
      output: "ALERT-high-risk.md"
      template: |
        # HIGH RISK ALERT

        **Score:** {{ processing.risk_assessment.score }}

        ## Immediate Actions

        {{ processing.risk_assessment.recommendation }}

        ## Matched Indicators

        {% for ind in processing.risk_assessment.matched_indicators %}
        - {{ ind.name }}: {{ ind.description }}
        {% endfor %}

    - name: data-export
      format: json
      output: "investigation-data.json"
```

## Output Location

Reports are written to the output folder:

```
output/
└── {pack_name}/
    └── {timestamp}/
        ├── executive-summary.md
        ├── technical-details.md
        └── data.json
```

## Related

- [Templates](templates.md) - Tera templating syntax
- [Scoring](../processing/scoring.md) - Using scoring results
- [Output](../acquisition/output.md) - Output folder configuration

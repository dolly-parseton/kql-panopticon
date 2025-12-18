# Processing: Scoring

The scoring step calculates risk scores based on weighted indicators from acquisition data.

## Schema

```yaml
processing:
  steps:
    - name: step_name         # Required: unique identifier
      type: scoring           # Required: must be "scoring"

      indicators:             # Required: scoring rules
        - name: indicator_name
          condition: "{{...}}"
          weight: 25
          description: "..."

      thresholds:             # Required: risk levels
        - level: "CRITICAL"
          min_score: 80
          summary: "..."
          recommendation: "..."
```

## Indicators

Indicators define conditions that contribute to the score.

```yaml
indicators:
  - name: high_failures
    condition: "{{signins | any(FailedCount > 10)}}"
    weight: 25
    description: "High failed login count"

  - name: suspicious_ip
    condition: "{{enrichment | any(threat_score > 80)}}"
    weight: 40
    description: "Connection from known malicious IP"
```

### Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Unique indicator identifier |
| `condition` | Yes | Boolean expression (must evaluate to true/false) |
| `weight` | Yes | Score contribution when triggered |
| `description` | No | Human-readable explanation |

### Weight Values

- **Positive weights**: Add to risk score (higher = more risk)
- **Negative weights**: Subtract from risk (mitigating factors)

```yaml
indicators:
  # Risk indicators (positive)
  - name: failed_logins
    condition: "{{signins | any(FailedCount > 5)}}"
    weight: 20

  # Mitigating indicators (negative)
  - name: known_device
    condition: "{{devices | any(IsManaged == true)}}"
    weight: -10
```

### Condition Expressions

Conditions must evaluate to boolean. Common patterns:

```yaml
# Step has results
condition: "{{step | is_not_empty}}"

# Any row matches predicate
condition: "{{step | any(field > value)}}"

# All rows match
condition: "{{step | all(status == 'active')}}"

# Row count comparison
condition: "{{step | length | gt(10)}}"

# Filtered count
condition: "{{step | filter(severity == 'High') | length | gt(0)}}"

# Compound conditions
condition: "{{step1 | any(score > 50)}} and {{step2 | is_not_empty}}"
```

## Thresholds

Thresholds define risk levels based on accumulated score.

```yaml
thresholds:
  - level: CRITICAL
    min_score: 80
    summary: "Critical risk detected (score: {{score}})"
    recommendation: "Immediate action required"

  - level: HIGH
    min_score: 50
    summary: "High risk detected (score: {{score}})"
    recommendation: "Investigate promptly"

  - level: MEDIUM
    min_score: 25
    summary: "Moderate risk (score: {{score}})"
    recommendation: "Monitor closely"

  - level: LOW
    min_score: 0
    summary: "Low risk (score: {{score}})"
    recommendation: "Continue monitoring"
```

### Fields

| Field | Required | Description |
|-------|----------|-------------|
| `level` | Yes | Risk level name |
| `min_score` | Yes | Minimum score for this level |
| `summary` | No | Template string (supports `{{score}}`) |
| `recommendation` | No | Suggested action |

### Threshold Matching

Thresholds are evaluated from highest `min_score` to lowest:
- Score 85 matches CRITICAL (min_score: 80)
- Score 60 matches HIGH (min_score: 50)
- Score 0 matches LOW (min_score: 0)

## Scoring Result

The scoring step produces a result available in reporting:

```yaml
processing:
  risk_assessment:
    score: 65
    level: "HIGH"
    matched_indicators:
      - name: "failed_logins"
        weight: 25
        description: "High failed login count"
      - name: "suspicious_ip"
        weight: 40
        description: "Connection from known malicious IP"
    summary: "High risk detected (score: 65)"
    recommendation: "Investigate promptly"
```

## Complete Example

```yaml
processing:
  steps:
    - name: risk_assessment
      type: scoring

      indicators:
        # Authentication indicators
        - name: has_failed_logins
          condition: "{{signins | any(FailedCount > 0)}}"
          weight: 15
          description: "User has failed login attempts"

        - name: high_failure_rate
          condition: "{{signins | any(FailureRate > 20)}}"
          weight: 25
          description: "Failure rate above 20%"

        # Threat indicators
        - name: suspicious_ip
          condition: "{{ip_activity | any(IsSuspicious == true)}}"
          weight: 30
          description: "Connection from suspicious IP"

        - name: high_threat_score
          condition: "{{enrichment | any(threat_score > 75)}}"
          weight: 40
          description: "IP has high threat score"

        # Alert indicators
        - name: high_severity_alert
          condition: "{{alerts | any(Severity == 'High')}}"
          weight: 35
          description: "High severity alert present"

        - name: multiple_alerts
          condition: "{{alerts | length | gt(3)}}"
          weight: 15
          description: "Multiple security alerts"

        # Mitigating factors
        - name: managed_device
          condition: "{{devices | any(IsManaged == true)}}"
          weight: -10
          description: "Using managed device"

      thresholds:
        - level: CRITICAL
          min_score: 80
          summary: "Critical risk: Multiple severe indicators (score: {{score}})"
          recommendation: "Immediate investigation required. Consider account suspension."

        - level: HIGH
          min_score: 50
          summary: "High risk: Significant concerns detected (score: {{score}})"
          recommendation: "Investigate user activity within 24 hours."

        - level: MEDIUM
          min_score: 25
          summary: "Medium risk: Some suspicious activity (score: {{score}})"
          recommendation: "Review recent activity and monitor."

        - level: LOW
          min_score: 0
          summary: "Low risk: Normal activity (score: {{score}})"
          recommendation: "No action required."
```

## Using in Reports

Access scoring results in report templates:

```jinja2
# Risk Assessment

**Score**: {{ processing.risk_assessment.score }}
**Level**: {{ processing.risk_assessment.level }}

## Summary
{{ processing.risk_assessment.summary }}

## Recommendation
{{ processing.risk_assessment.recommendation }}

{% if processing.risk_assessment.matched_indicators %}
## Matched Indicators
{% for ind in processing.risk_assessment.matched_indicators %}
- **{{ ind.name }}** (+{{ ind.weight }}): {{ ind.description }}
{% endfor %}
{% endif %}
```

## Using in Conditions

Scoring results can drive conditional reporting:

```yaml
reporting:
  reports:
    - name: critical_alert
      when: "{{processing.risk_assessment.level | first | eq('CRITICAL')}}"
      template: |
        # CRITICAL ALERT
        ...
```

## Related

- [Reporting Overview](../reporting/overview.md) - Using scoring in reports
- [Variable Syntax](../reference/variable-syntax.md) - Condition expressions
- [Predicates](../reference/predicates.md) - Row filtering

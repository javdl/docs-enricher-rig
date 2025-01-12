filter priority:
    jq '.[] | {filepath, issue: (.issues[] | select(.priority == "{{priority}}"))}' result.json

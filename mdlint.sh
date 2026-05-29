#!/bin/bash
# markdown の lint および mermaid チェック
markdownlint-cli2 "**/*.md" --fix && mermaid-validate -q "**/*.md"
# 以下の mermaid linter は processing heavy
# md-mermaid-lint "**/*.md"
#!/bin/bash
markdownlint-cli2 "**/*.md" --fix

# mermaid のチェック必要なら以下のツールを導入すること
#  && mermaid-validate "**/*.md"
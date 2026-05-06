#!/bin/bash
find ./crates -name "*.rs" -type f -exec wc -l {} + | sort -nr | head -30
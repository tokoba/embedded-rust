#!/bin/bash
# commit and auto generate commit messages(by `git-cc -y`) and
# push to remote
git add . && git-cc -y && git push origin main

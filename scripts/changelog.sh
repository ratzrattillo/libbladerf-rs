#!/bin/bash

# Bash debug mode
set -xe

# Run from the repository root so relative paths (cliff.toml, CHANGELOG.md)
# resolve regardless of the invocation directory.
cd "$(dirname "$0")/.."

###########################################################
# CHANGELOG
###########################################################
# Generate changelog entries for unreleased commits (since the latest tag)
# and prepend them to CHANGELOG.md. Mutates CHANGELOG.md in place, so run
# this on demand rather than as part of check.sh.
git-cliff --unreleased --prepend CHANGELOG.md

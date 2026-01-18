#!/usr/bin/env bash
# Wrapper to run Python threads module without changing directory
# This allows tests to work from their own working directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHONPATH="$SCRIPT_DIR/src" exec python -m threads "$@"

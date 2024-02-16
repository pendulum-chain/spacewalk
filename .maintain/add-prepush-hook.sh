#!/bin/sh

# write a whole script to pre-push hook
# NOTE: it will overwrite pre-push file!
cat > .git/hooks/pre-push <<'EOF'
#!/bin/bash

echo "Running clippy checks"

set +e

cargo clippy --all-features -- -W clippy::all -A clippy::style -A clippy::forget_copy -A clippy::forget_ref
CLIPPY_EXIT_CODE=$?

set -e

if [ $CLIPPY_EXIT_CODE -ne 0 ]; then
    echo "Error: Clippy checks failed" >&2
    exit $CLIPPY_EXIT_CODE
fi

echo "Clippy checks successful"

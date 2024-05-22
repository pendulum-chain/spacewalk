#!/bin/sh

# write a whole script to pre-push hook
# NOTE: it will overwrite pre-push file!
cat > .git/hooks/pre-push <<'EOF'
#!/bin/bash

echo "Running clippy checks"

set +e

echo "Running clippy for main targets"
chmod +x ./scripts/cmd-all
./scripts/cmd-all clippy "clippy --lib --bins" "-- -W clippy::all -A clippy::style -A forgetting_copy_types -A forgetting_references" 

echo "Running clippy for all targets"
cargo +nightly-2024-04-18 clippy --all-features --all-targets -- -A clippy::all -W clippy::correctness -A clippy::forget_copy -A clippy::forget_ref

CLIPPY_EXIT_CODE=$?

set -e

if [ $CLIPPY_EXIT_CODE -ne 0 ]; then
    echo "Error: Clippy checks failed" >&2
    exit $CLIPPY_EXIT_CODE
fi

echo "Clippy checks successful"

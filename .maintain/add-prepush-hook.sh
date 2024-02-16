#!/bin/sh

# check that rustfmt is installed, or else this hook doesn't make much sense
command -v rustfmt >/dev/null 2>&1 || { echo >&2 "Rustfmt is required but it's not installed. Aborting."; exit 1; }

# write a whole script to pre-push hook
# NOTE: it will overwrite pre-push file!
cat > .git/hooks/pre-push <<'EOF'
#!/bin/bash -e

echo "Running cargo clippy --fix on project"
before_clippy=$(git diff --name-only)
cargo clippy --fix --allow-dirty --allow-staged --all-features
wait
after_clippy=$(git diff --name-only)


before_files=()
while IFS= read -r line; do
    before_files+=("$line")
done <<< "$before_clippy"

after_files=()
while IFS= read -r line; do
    after_files+=("$line")
done <<< "$after_clippy"

clippy_modified_files=()
for i in "${after_files[@]}"; do
    skip=
    for j in "${before_files[@]}"; do
        [[ $i == $j ]] && { skip=1; break; }
    done
    [[ -n $skip ]] || clippy_modified_files+=("$i")
done

# Adds only the files modified by clippy
if [ ${#clippy_modified_files[@]} -ne 0 ]; then
    git add "${clippy_modified_files[@]}"
    git commit -m "Apply automatic cargo clippy fixes"
    echo "Clippy modifications added for: ${clippy_modified_files[*]}"
else
    echo "No modifications by clippy"
fi
wait 
echo "Pre-push hook execution completed."
EOF

chmod +x .git/hooks/pre-push

echo "Hooks updated"

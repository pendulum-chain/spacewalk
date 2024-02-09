#!/bin/sh

# check that rustfmt installed, or else this hook doesn't make much sense
command -v rustfmt >/dev/null 2>&1 || { echo >&2 "Rustfmt is required but it's not installed. Aborting."; exit 1; }

# write a whole script to pre-commit hook
# NOTE: it will overwrite pre-commit file!
cat > .git/hooks/pre-commit <<'EOF'
#!/bin/bash -e
declare -a rust_files=()
files=$(git diff --name-only --staged)
echo 'Formatting source files'
for file in $files; do
    if [ ! -f "${file}" ]; then
        continue
    fi
    if [[ "${file}" == *.rs ]]; then
        rust_files+=("${file}")
    fi
done
if [ ${#rust_files[@]} -ne 0 ]; then
    command -v rustfmt >/dev/null 2>&1 || { echo >&2 "Rustfmt is required but it's not installed. Aborting."; exit 1; }
    $(command -v rustfmt) ${rust_files[@]} &
fi
wait
if [ ${#rust_files[@]} -ne 0 ]; then
    git add ${rust_files[@]}
    echo "Formatting done, changed files: ${rust_files[@]}"
else
    echo "No changes, formatting skipped"
fi
wait

echo "Running cargo clippy --fix on project"
before_clippy=$(git diff --name-only)
$(command -v cargo) clippy --fix --allow-dirty --allow-staged --all-features
after_clippy=$(git diff --name-only)
wait

# Files modified by clippy
readarray -t before_files <<< "$before_clippy"
readarray -t after_files <<< "$after_clippy"
clippy_modified_files=()
for i in "${after_files[@]}"; do
    skip=
    for j in "${before_files[@]}"; do
        [[ $i == $j ]] && { skip=1; break; }
    done
    [[ -n $skip ]] || clippy_modified_files+=("$i")
done

# Add only the files modified by clippy
if [ ${#clippy_modified_files[@]} -ne 0 ]; then
    git add ${clippy_modified_files[@]}
    echo "Clippy modifications added for: ${clippy_modified_files[@]}"
else
    echo "No modifications by clippy"
fi

echo "Pre-commit hook execution completed."

EOF

chmod +x .git/hooks/pre-commit

echo "Hooks updated"

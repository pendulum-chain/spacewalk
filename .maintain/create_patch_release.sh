#!/bin/bash

# Find all Cargo.toml files in the current directory and its subdirectories
for file in $(find . -name "Cargo.toml")
do
    # Use awk to increment the version number of the package
    awk -F'.' '/\[package\]/,/version =/ { if($0 ~ /version =/ && $0 !~ /#/) {print $1"."$2"."$3+1"\""; next} }1' $file > temp && mv temp $file
done
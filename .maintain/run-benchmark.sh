#!/bin/sh

echo $1
cd ..
./target/release/spacewalk-standalone benchmark pallet \
--chain=dev \
--pallet="$1" \
--extrinsic='*' \
--steps=100 \
--repeat=10 \
--wasm-execution=compiled \
--output="pallets/$1/src/default_weights.rs" \
--template=.maintain/frame-weight-template.hbs

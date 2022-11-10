#!/bin/sh

cd "pallets"
for dir in */; do
  pallet=${dir%"/"}
  ../.maintain/run-benchmark.sh $pallet
done

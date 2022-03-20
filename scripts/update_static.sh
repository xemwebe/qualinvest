#!/usr/bin/bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd ${SCRIPT_DIR}/..
cd wasm_graph
wasm-pack build --release --target web
cd ..

rm -rf static
mkdir static
cp ./qualinvest_server/static/* static
cp ./wasm_graph/pkg/wasm_graph_bg.wasm static
cp ./wasm_graph/pkg/wasm_graph.js static

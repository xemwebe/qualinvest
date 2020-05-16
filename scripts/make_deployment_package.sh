#!/usr/bin/bash

cargo build --release
mkdir -p dist_package
cd dist_package
mkdir -p bin
mkdir -p static
mkdir -p templates
mkdir -p config
cp ../target/release/qualinvest_cli bin
cp ../target/release/qualinvest_server bin
cp -R ../qualinvest_server/static/* static
cp -R ../qualinvest_server/templates/*.tera templates
cp ../qualinvest_template.toml config

cd ..
tar czvf qualinvest.tar.gz dist_package/*
rm -rf dist_package


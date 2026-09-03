#!/bin/sh
wasm-pack build --release --target web
cp pkg/*.ts lib/web
cp pkg/*.js lib/web
cp pkg/*.wasm lib/web
cp pkg/README.md lib/web
cp pkg/LICENSE* lib/web
cd lib/web
npm install
npm run prepare

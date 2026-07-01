# Microscope Memory — Web Demo

A browser-based demo of Microscope Memory.

## Try it

Open `index.html` in your browser, or visit the GitHub Pages deployment.

## How it works

The demo loads a WASM build of Microscope Memory directly in the browser.
No server required. All data is processed locally.

## Build

```bash
wasm-pack build --target web --out-dir web-demo/pkg -- --features wasm
```

## Safety

- Read-only mode
- No data leaves your browser
- No tracking
- MIT License

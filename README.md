# 🕵️‍♂️ stegowasm

A high-performance **Rust + WebAssembly steganography library** for browser apps.

Embed and extract hidden text inside PNG images using **LSB steganography**, with optional **AES-256-GCM encryption** and **PBKDF2 key derivation** — all running safely off the UI thread via Web Workers.

---

## ✨ Features

- 🖼️ PNG steganography (LSB, RGB8)
- 🔐 AES-256-GCM encryption
- 🔑 PBKDF2-HMAC-SHA256 key derivation
- 🧠 Automatic compression fallback (Zlib)
- ⚡ WebAssembly (Rust) for performance
- 🧵 Web Worker friendly (non-blocking UI)
- 📦 Easy integration with Angular / React / any frontend

---

## 📦 Installation

```bash
npm install stegowasm
```

or (scoped):

```bash
npm install @your-scope/stegowasm
```

---

## 🚀 Usage

### Initialize

```ts
import init, { embed_text, extract_text } from 'stegowasm';

await init();
```

---

### 🔒 Embed text into image

```ts
const inputBytes = new Uint8Array(await file.arrayBuffer());

const outputBytes = embed_text(
  inputBytes,
  'Secret message',
  true, // use encryption
  'my-password'
);

const blob = new Blob([outputBytes], { type: 'image/png' });
```

---

### 🔍 Extract text from image

```ts
const inputBytes = new Uint8Array(await file.arrayBuffer());

const text = extract_text(
  inputBytes,
  true, // use encryption
  'my-password'
);

console.log(text);
```

---

### 📏 Estimate capacity

```ts
import { estimate_capacity } from 'stegowasm';

const capacity = estimate_capacity(inputBytes);
console.log(`Max payload size: ${capacity} bytes`);
```

---

## 🧵 Web Worker usage (recommended)

For large images or strong encryption (PBKDF2), always run inside a worker:

```ts
const worker = new Worker(new URL('./steganography.worker', import.meta.url), {
  type: 'module',
});
```

This keeps your UI responsive while processing images.

---

## 🔐 Encryption details

- Algorithm: **AES-256-GCM**
- Key derivation: **PBKDF2-HMAC-SHA256**
- Iterations: `600,000`
- Format:

```
salt (16 bytes) || nonce (12 bytes) || ciphertext + tag
```

---

## 🧠 Compression strategy

The library automatically optimizes payload size:

1. Try raw text
2. If it does not fit → compress (Zlib)
3. If still does not fit → throw error

---

## 📊 Capacity

Capacity depends on image size:

```
capacity = image_bytes / 8 - header
```

- Each byte stores 1 bit (LSB)
- Header size: 5 bytes

---

## ⚠️ Limitations

- ❌ JPEG is not supported (lossy compression destroys data)
- ⚠️ Large images may take time (use Web Worker)

---

## 🛠️ Development

### Build WASM

```bash
wasm-pack build --target bundler --release
```

---

### Local usage

```bash
npm install ../stegowasm/pkg
```

---

## 🔄 Versioning

Follow semantic versioning:

- `patch` → bug fixes
- `minor` → new features (backward compatible)
- `major` → breaking changes (format/encryption changes)

---

## 🚀 Roadmap

- [ ] Magic/version header (format detection)
- [ ] Streaming for large files
- [ ] Argon2id support
- [ ] Worker pool support

---

## 🧑‍💻 Author

Zoran Davidović

---

## 📄 License

MIT

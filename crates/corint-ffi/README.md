# CORINT FFI - Foreign Function Interface

Multi-language bindings for the CORINT Decision Engine (Python, Go, TypeScript/Node.js, Java).

## Supported Languages

- **Python** (ctypes)
- **Go** (cgo)
- **TypeScript/Node.js** (napi-rs)
- **Java** (JNA)

## Build the FFI Library

```bash
cargo build -p corint-ffi --release
```

Outputs:
- macOS: `target/release/libcorint_ffi.dylib`
- Linux: `target/release/libcorint_ffi.so`
- Windows: `target/release/corint_ffi.dll`

 

## Run Examples

### Python

Run from repo root so `./repository` is found:

```bash
python3 crates/corint-ffi/bindings/python/example.py
```

### Go

```bash
cd crates/corint-ffi/bindings/go
go run example/main.go
```

### TypeScript / Node.js

```bash
cd crates/corint-ffi/bindings/typescript
npm install
npm run build
npm run example
npm run example:ts
```

### Java

Create a local symlink to the repository, then run the example:

```bash
cd crates/corint-ffi/bindings/java
ln -s ../../../../repository repository
mvn -q -Dexec.mainClass=com.corint.Example -Dexec.classpathScope=runtime \
  org.codehaus.mojo:exec-maven-plugin:3.1.0:java
```

If JNA cannot locate the native library, export one of:

```bash
export DYLD_LIBRARY_PATH=../../../../target/release   # macOS
export LD_LIBRARY_PATH=../../../../target/release     # Linux
```

## Troubleshooting

- **Library not found**: ensure `cargo build -p corint-ffi --release` has been run.
- **Import not found**: run examples from repo root or ensure `repository/` is available.
- **Missing features**: provide `features` in the request or configure datasources under `repository/configs/datasources`.

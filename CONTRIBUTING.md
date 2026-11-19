## Contributing

Thank you for in contributing! This app have three main parts:

- **App UI** - Android activities in Kotlin and XML.
- **HTTP Server** - the Rust HTTP core (`transfer-core/`, via JNI) that the app run.
- **Website** - Plain HTML/JS/CSS served from the app's assets.

Please follow the instructions below to get started.

---

### Website Editing

Website files are in `app/src/main/assets/` and can be edited like any regular HTML/CSS/JS project.

To simplify development:

1. Forward the app's server to your computer (you need to run Transfer on emulator for this script to work):

```bash
adb forward tcp:9135 tcp:8000
```

2. Start the local dev server with live reload:
Install server dependencies:
```bash
pip install requests flask livereload urllib3
```
Then run
```bash
python serve.py localhost:9135
```

3. Visit [http://localhost:4444](http://localhost:4444) to see your changes.

This setup live-reloads the site and forwards `/api/*` requests to the emulator.

### App UI (XML/Kotlin)

- Themes are defined in `res/values/themes.xml` and `res/values/colors.xml`.
- For dark mode: check `res/values-night/colors.xml`.
- Layouts are in `res/layout/`. the UI code in `src/main/java/.../ui/`, and MainActivity is in `src/main/java/.../MainActivity`.

### HTTP Server (Rust)

- The HTTP implementation lives in `transfer-core/` (Rust, axum). Rebuild it with:

```bash
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o app/src/main/jniLibs build --release
```

`./gradlew assembleDebug` runs this for you via the `cargoNdk` task.
API endpoints (such as /api/upload): `transfer-core/src/http.rs`
Android <-> Rust bridge: `server/RustServer.kt` and `server/SafBridge.kt`
Server features (like password protection): `server/FileServerService`

---

### PR Guidelines

- for code changes, Use commit messages with tags, e.g.:

  - `fix(server): handle large file uploads`
  - `feat(main): add new button`
  - `tests: add tests for upload endpoint`

- Fork the repo and create a branch for your change.
- Open a pull request

Looking forward to your contributions!

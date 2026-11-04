# meROMS

Desktop ROM manager for EmulationStation `gamelist.xml` files. Browse your systems, edit game
metadata, manage media, scrape from online databases and keep your library tidy. It supports the
classic EmulationStation folder structure and ES-DE, and can migrate a library between them.

Built with [Tauri v2](https://tauri.app/) (Rust) + React 19 + TypeScript + Tailwind CSS.
The UI is available in English and Spanish.

---

## Building the app from source

The app has to be built **on the same operating system it is for**: build the Windows installer
on Windows, the macOS app on macOS and the Linux packages on Linux. Cross-compiling is not
supported by this guide.

### 1. Install the prerequisites

Every platform needs:

| Tool | Version | Why |
|---|---|---|
| [Git](https://git-scm.com/downloads) | any recent | to download the project |
| [Node.js](https://nodejs.org/) | **22 LTS** (20.19+ also works) | builds the user interface (Vite 7) |
| [pnpm](https://pnpm.io/) | 10 or newer | installs the JavaScript dependencies |
| [Rust](https://www.rust-lang.org/tools/install) (via `rustup`) | latest stable | builds the native app |

Recommended way to get pnpm: it ships with Node.js through Corepack, so after installing Node.js
just run:

```bash
corepack enable
corepack prepare pnpm@latest --activate
pnpm --version
```

Then the platform-specific system libraries:

#### macOS

1. Install the Xcode Command Line Tools (compiler and SDK):

   ```bash
   xcode-select --install
   ```

2. Install Rust:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source "$HOME/.cargo/env"
   ```

3. Install Node.js 22 LTS from [nodejs.org](https://nodejs.org/) or with Homebrew
   (`brew install node@22`), then run the Corepack commands above.

#### Windows (10 / 11)

1. Install the **Microsoft C++ Build Tools**: download
   [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) and,
   in the installer, tick the **"Desktop development with C++"** workload.
2. **WebView2**: already included in Windows 10 (version 1803 or later) and Windows 11. If it is
   missing, install the "Evergreen Bootstrapper" from
   [Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/).
3. Install Rust with [rustup-init.exe](https://www.rust-lang.org/tools/install) and keep the
   default **MSVC** toolchain (`stable-x86_64-pc-windows-msvc`).
4. Install Node.js 22 LTS from [nodejs.org](https://nodejs.org/) (or
   `winget install OpenJS.NodeJS.LTS`), then run the Corepack commands above in a new terminal.
5. Install Git from [git-scm.com](https://git-scm.com/download/win) (or `winget install Git.Git`).

Close and reopen the terminal (PowerShell) after installing everything so the new `PATH` is
picked up. The installer tools (NSIS and WiX) are downloaded automatically by Tauri the first
time you build.

#### Linux

Install the WebKitGTK libraries and build tools for your distribution:

**Debian / Ubuntu / Linux Mint** (Ubuntu 22.04 or newer):

```bash
sudo apt update
sudo apt install -y git curl wget file build-essential pkg-config \
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

**Fedora**:

```bash
sudo dnf check-update
sudo dnf install -y git curl wget file openssl-devel \
  webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel libxdo-devel
sudo dnf group install -y "c-development"
```

**Arch Linux / Manjaro**:

```bash
sudo pacman -Syu --needed git curl wget file base-devel openssl \
  webkit2gtk-4.1 appmenu-gtk-module libappindicator-gtk3 librsvg xdotool
```

Then install Rust and Node.js:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
# Node.js 22 LTS: use your distro's package, nvm or https://nodejs.org/
```

and run the Corepack commands above.

#### Check that everything is in place

```bash
git --version
node --version     # v22.x (or v20.19+)
pnpm --version     # 10.x or newer
rustc --version    # rustc 1.8x or newer
cargo --version
```

### 2. Download the project

```bash
git clone https://github.com/marcosesperon/meROMS.git
cd meROMS
```

To update an existing copy later: `git pull`.

### 3. Install the dependencies

```bash
pnpm install
```

This installs the JavaScript packages (including the Tauri CLI). The Rust crates are downloaded
automatically during the first build.

### 4. Build the app

```bash
pnpm tauri build
```

The first build takes several minutes (it compiles all the Rust dependencies); later builds are
much faster. When it finishes, the results are in `src-tauri/target/release/bundle/`:

| Platform | What you get | Where |
|---|---|---|
| **macOS** | App bundle | `bundle/macos/meROMS.app` |
| | Disk image (installer) | `bundle/dmg/meROMS_0.1.0_<arch>.dmg` |
| **Windows** | Setup program (recommended) | `bundle/nsis/meROMS_0.1.0_x64-setup.exe` |
| | MSI installer | `bundle/msi/meROMS_0.1.0_x64_en-US.msi` |
| **Linux** | Debian/Ubuntu package | `bundle/deb/meROMS_0.1.0_amd64.deb` |
| | Fedora/openSUSE package | `bundle/rpm/meROMS-0.1.0-1.x86_64.rpm` |
| | Portable AppImage | `bundle/appimage/meROMS_0.1.0_amd64.AppImage` |

The bare executable (without installer) is also at `src-tauri/target/release/meroms`
(`meroms.exe` on Windows).

#### Building only some formats

By default every format of the current OS is built. To build just one (faster):

```bash
pnpm tauri build --bundles dmg            # macOS: only the .dmg (app)
pnpm tauri build --bundles nsis           # Windows: only the -setup.exe
pnpm tauri build --bundles deb,appimage   # Linux: .deb and AppImage
```

#### macOS: one app for Intel and Apple Silicon

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm tauri build --target universal-apple-darwin
```

The universal app is left in `src-tauri/target/universal-apple-darwin/release/bundle/`.

### 5. Install and run

- **macOS**: open the `.dmg` and drag **meROMS** to Applications. The build is not signed, so the
  first time macOS will block it: right-click the app > **Open** > **Open**, or run
  `xattr -dr com.apple.quarantine /Applications/meROMS.app`.
- **Windows**: run `meROMS_0.1.0_x64-setup.exe`. As the installer is not signed, SmartScreen may
  warn: click **More info** > **Run anyway**.
- **Linux**:
  - `.deb`: `sudo apt install ./meROMS_0.1.0_amd64.deb`
  - `.rpm`: `sudo dnf install ./meROMS-0.1.0-1.x86_64.rpm`
  - AppImage: `chmod +x meROMS_0.1.0_amd64.AppImage && ./meROMS_0.1.0_amd64.AppImage`

On first launch the app opens the Settings window so you can create a profile (your ROM folder
and its structure: Classic or ES-DE).

### Troubleshooting

- **`pnpm: command not found`**: run `corepack enable` again, or install pnpm with
  `npm install -g pnpm`.
- **Windows: `link.exe not found` / MSVC errors**: the "Desktop development with C++" workload of
  the Build Tools is missing (step 1 of the Windows prerequisites).
- **Linux: `webkit2gtk-4.1` not found**: your distribution is too old for Tauri v2 (Ubuntu 22.04+,
  Debian 12+ or Fedora 36+ are needed), or the `-dev` packages above are not installed.
- **Strange build errors after updating**: delete the build caches and try again:
  `rm -rf node_modules src-tauri/target && pnpm install && pnpm tauri build`.

---

## Automated builds (GitHub Actions)

Instead of building on each operating system yourself, GitHub can build all the installers on
its own Windows, macOS and Linux machines. The workflow is in `.github/workflows/build.yml`.

1. (Optional) Bump the version in `src-tauri/tauri.conf.json` (`"version"`), commit and push.
   The Release is named after that version, so with the same version the same Release is reused.
2. On GitHub, open the **Actions** tab > **Build installers** > **Run workflow** (branch `main`).
3. Wait for the three jobs (macOS, Windows, Linux) to finish. The first run takes about
   15-25 minutes; later runs are faster thanks to the build cache.
4. Open **Releases**: there is a **draft** called `meROMS v<version>` with every installer
   attached:
   - macOS: one universal `.dmg` (Intel and Apple Silicon)
   - Windows: `-setup.exe` and `.msi`
   - Linux: `.deb`, `.rpm` and `.AppImage`
5. Check it and press **Publish release** to make it public. Until then only you can see it.

The installers are not signed, so the notes in "Install and run" above apply. Actions minutes are
free for public repositories; for private ones they count against your plan (macOS minutes x10,
Windows x2).

---

## Development

```bash
pnpm tauri dev                                  # app with hot reload (Vite + Rust + window)
pnpm dev                                        # interface only, in the browser (no Tauri APIs)
pnpm build                                      # type-check and build the interface
cargo test --manifest-path src-tauri/Cargo.toml # Rust unit tests
```

Recommended editor: [VS Code](https://code.visualstudio.com/) with the
[Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) and
[rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
extensions.

Architecture notes for contributors are in `CLAUDE.md`.

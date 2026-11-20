# Reply Timer

## Get the .exe (no Rust needed)
1. Create a GitHub repo and push this folder to it.
2. Open the repo's **Actions** tab -> "Build Windows EXE" -> latest run -> download **ReplyTimer** (contains ReplyTimer.exe).
   Or tag a release (`git tag v1.0 && git push --tags`) and download ReplyTimer.exe from the Releases page.

## Build it yourself on Windows
Install Rust from https://rustup.rs, then: `cargo build --release`
The single file is at `target\release\reply_timer_gui.exe`.

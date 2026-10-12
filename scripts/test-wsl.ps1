# Run the Rust test suite inside WSL (Ubuntu), for when Windows Smart App
# Control blocks freshly built test binaries ("An Application Control policy
# has blocked this file"). Needs, once, inside WSL:
#   sudo apt-get install -y build-essential pkg-config libssl-dev curl
#   curl https://sh.rustup.rs -sSf | sh -s -- -y && ~/.cargo/bin/rustup default stable
# Builds go to ~/toad-target inside WSL so they don't clash with the Windows build.
$repo = (Resolve-Path "$PSScriptRoot\..").Path
$wslPath = (wsl wslpath -a ($repo -replace '\', '/')).Trim()
wsl -e bash -lc "cd '$wslPath' && CARGO_TARGET_DIR=~/toad-target ~/.cargo/bin/cargo test --workspace --exclude screaming-toad $args"

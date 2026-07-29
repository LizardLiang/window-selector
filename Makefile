# Makefile for window-selector
# Builds the Rust binary and packages it into a Windows installer via NSIS.
#
# Prerequisites:
#   - Rust toolchain (stable, target x86_64-pc-windows-gnu — see .cargo/config.toml)
#   - NSIS 3.x (makensis on PATH)
#   - PowerShell 5.1+ (ships with Windows 10/11)
#   - GNU Make (or run cargo and makensis commands manually)
#
# Usage:
#   make build      - Compile release binary only
#   make installer  - Compile release binary then build installer .exe
#   make clean      - Remove build artifacts

VERSION := $(shell powershell -NoProfile -Command \
  "(Select-String -Path Cargo.toml -Pattern '^version\s*=\s*\"(.+)\"').Matches.Groups[1].Value")

NSIS  := makensis
CARGO := cargo

# Cargo target triple. Must match .cargo/config.toml, and is passed through to
# installer.nsi so the installer never has to guess where the binary landed.
TARGET := x86_64-pc-windows-gnu

.PHONY: build installer clean

build:
	$(CARGO) build --release

installer: build
	$(NSIS) /DVERSION=$(VERSION) /DTARGET=$(TARGET) installer/installer.nsi
	@echo "Installer created: target/$(TARGET)/release/WindowSelector-$(VERSION)-setup.exe"

clean:
	$(CARGO) clean

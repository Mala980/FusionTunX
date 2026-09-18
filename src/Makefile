.PHONY: build dev clean help deps setup-dev

help:
	@echo "FusionTunX - Backend API untuk aplikasi tunneling menggunakan mihomo (Rust)"
	@echo ""
	@echo "Available commands:"
	@echo "  build        - Build aplikasi dengan embedded dashboard (1 binary)"
	@echo "  dev          - Jalankan development server (cargo run)"
	@echo "  clean        - Hapus build artifacts dan frontend build"
	@echo "  deps         - Check Rust dependencies"
	@echo "  help         - Tampilkan pesan bantuan ini"
	@echo ""
	@echo "Usage examples:"
	@echo "  make build              # Build 1 binary dengan dashboard embedded"
	@echo "  make dev                # Development server"
	@echo "  ./bin/fusiontunx -c /path/to/config.yaml  # Production dengan custom config"

build:
	@echo "=== Building FusionTunX (Rust) ==="
	@echo "[1/2] Building frontend..."
	@cd dash && npm install && npm run build
	@echo "[2/2] Building Rust binary with embedded dashboard..."
	@mkdir -p bin
	@cargo build --release
	@if [ -f "target/release/fusiontunx" ]; then cp target/release/fusiontunx bin/fusiontunx; \
	 elif [ -f "../target/release/fusiontunx" ]; then cp ../target/release/fusiontunx bin/fusiontunx; fi
	@echo ""
	@echo "✓ Build complete! Binary: bin/fusiontunx"

dev:
	@echo "Starting development server..."
	cargo run

clean:
	@echo "Cleaning..."
	rm -rf bin/
	rm -rf target/
	rm -rf dash/dist
	rm -rf dash/node_modules

deps:
	@echo "Checking dependencies..."
	cargo check

setup-dev: deps
	@echo "Development environment setup complete!"

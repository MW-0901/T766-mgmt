rpi_build:
	docker run --rm \
	  -u $(id -u):$(id -g) \
	  --platform linux/arm64 \
	  -e CARGO_HOME=/tmp/cargo \
	  -e HOME=/tmp \
	  -v .:/workspace \
	  -w /workspace \
	  ghcr.io/lewimbes/dioxus-docker:0.7.2 \
	  dx bundle --release -p T766-ControlServer

agent:
	@if not exist "C:\Program Files (x86)\WiX Toolset v3.11\bin\candle.exe" ( \
		echo ERROR: WiX Toolset not found. & \
		echo Expected: C:\Program Files ^(x86^)\WiX Toolset v3.11\bin\candle.exe & \
		echo Install WiX v3.11.2 from: & \
		echo https://github.com/wixtoolset/wix3/releases/tag/wix3112rtm & \
		exit /b 1 \
	)
	cargo install cargo-wix
	cargo build --release -p T766-ControlClient
	cargo wix -p T766-ControlClient \
		--bin-path "C:\Program Files (x86)\WiX Toolset v3.11\bin" \
		--nocapture
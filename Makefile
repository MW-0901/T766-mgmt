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

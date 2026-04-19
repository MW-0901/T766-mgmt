.PHONY: all server-x86 server-arm client clean

BUILD_DIR := build

all: server-x86 server-arm client

# ── Control Server (x86_64-linux) ─────────────────────────────────
server-x86:
	nix build .#controlServer -o $(BUILD_DIR)/server-x86
	@echo "Built x86_64 server → $(BUILD_DIR)/server-x86/"

# ── Control Server (aarch64-linux / Raspberry Pi) ─────────────────
server-arm:
	nix build .#controlServer-rpi -o $(BUILD_DIR)/server-arm
	@echo "Built aarch64 server → $(BUILD_DIR)/server-arm/"

# ── Windows Client + NSIS Installer ──────────────────────────────
client:
	nix build .#windowsClient -o $(BUILD_DIR)/client
	@echo "Built Windows client → $(BUILD_DIR)/client/"

# ── Housekeeping ──────────────────────────────────────────────────
clean:
	rm -f $(BUILD_DIR)/server-x86 $(BUILD_DIR)/server-arm $(BUILD_DIR)/client
	@echo "Cleaned build symlinks"

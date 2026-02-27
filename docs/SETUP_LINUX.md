# 🔥 Carnelian OS — Linux Setup Guide (Ubuntu/Debian)

Carnelian runs natively on Linux with full GPU support via the NVIDIA Container Toolkit.

---

## Prerequisites Checklist

| Requirement | Version | Notes |
|-------------|---------|-------|
| Ubuntu/Debian | 20.04+ / Debian 11+ | Other distros work with equivalent packages |
| Docker | 20.10+ | With Docker Compose |
| NVIDIA GPU drivers | 535+ | For GPU passthrough (optional) |
| NVIDIA Container Toolkit | Latest | For GPU in Docker containers |
| Rust | 1.85+ | Native toolchain |
| Node.js | 18+ | For Node.js worker |
| Python | 3.10+ | For Python worker |

---

## Step-by-Step Setup

### 1. Install Docker

```bash
sudo apt update
sudo apt install -y docker.io docker-compose-plugin

# Add your user to the docker group (avoids sudo for docker commands)
sudo usermod -aG docker $USER
newgrp docker

# Verify
docker --version
docker compose version
```

> **Note:** If using `docker-compose` (v1), install it separately: `sudo apt install -y docker-compose`. The v2 plugin (`docker compose`) is recommended.

### 2. Install NVIDIA Drivers (if GPU present)

```bash
# Check if a GPU is present
lspci | grep -i nvidia

# Install the latest driver
sudo apt install -y nvidia-driver-535  # Or latest version for your GPU
sudo reboot

# Verify after reboot
nvidia-smi
```

### 3. Install NVIDIA Container Toolkit

This enables GPU passthrough to Docker containers (required for Ollama GPU acceleration).

```bash
# Add the NVIDIA Container Toolkit repository
curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | \
  sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg

curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | \
  sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
  sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list

# Install
sudo apt update
sudo apt install -y nvidia-container-toolkit

# Configure Docker runtime
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker
```

### 4. Verify GPU in Docker

```bash
# Test GPU passthrough
docker run --rm --gpus all nvidia/cuda:12.0-base nvidia-smi
```

You should see your GPU listed. If not, check:
- Driver version: `nvidia-smi` on host
- Docker runtime: `docker info | grep -i runtime` should show `nvidia`
- Toolkit config: `cat /etc/docker/daemon.json` should reference `nvidia`

### 5. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify
rustc --version
cargo --version
```

### 6. Install Node.js and Python

```bash
# Node.js (via NodeSource for latest LTS)
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs

# Python
sudo apt install -y python3 python3-pip python3-venv

# Verify
node --version
npm --version
python3 --version
```

### 7. Install Development Tools

```bash
# SQLx CLI for database migrations
cargo install sqlx-cli --no-default-features --features postgres

# Pre-commit hooks
cargo install prek
```

---

## Carnelian First Run

```bash
# 1. Clone repository
git clone https://github.com/kordspace/carnelian.git
cd carnelian

# 2. Build the project
cargo build --release

# 3. Run the interactive setup wizard
# This detects your hardware, sets up Docker containers, creates the database,
# generates your owner keypair, and activates starter skills.
# GPU VRAM detection will determine whether you get 'performance' (≥10GB) or 'standard' (≥6GB).
carnelian init

# 4. Start the system
carnelian start
```

> **Non-interactive (CI/scripted):** `carnelian init --non-interactive`
> 
> For automated deployments, use the `--non-interactive` flag to skip all prompts.
> See [INSTALL.md](INSTALL.md) for detailed CI/CD setup options.

### Post-Init Setup

After running `carnelian init`, your system is ready. The wizard automatically:
- Starts PostgreSQL and Ollama Docker containers
- Runs database migrations
- Pulls appropriate models for your GPU profile (`deepseek-r1:7b` for Standard, `deepseek-r1:32b` for Performance)
- Generates and secures your owner keypair
- Activates starter skills

Access the dashboard at: http://localhost:18789

### Linux-Specific Notes

- **NVIDIA GPU users**: The wizard will detect your VRAM and suggest the optimal profile
- **CPU-only users**: You'll get the "custom" profile with a 7B model
- ** systemd**: For production deployments, consider setting up systemd services for auto-start

---

## Systemd Service (Optional)

Run Carnelian as a systemd service for automatic startup on boot.

### Create the Service File

```bash
sudo tee /etc/systemd/system/carnelian.service > /dev/null << 'EOF'
[Unit]
Description=Carnelian OS Orchestrator
After=network.target docker.service
Requires=docker.service

[Service]
Type=simple
User=carnelian
Group=carnelian
WorkingDirectory=/opt/carnelian
Environment=DATABASE_URL=postgresql://carnelian:carnelian@localhost:5432/carnelian
Environment=CARNELIAN_HTTP_PORT=18789
Environment=RUST_LOG=info
ExecStart=/opt/carnelian/target/release/carnelian start
ExecStop=/opt/carnelian/target/release/carnelian stop
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
```

### Enable and Start

```bash
sudo systemctl daemon-reload
sudo systemctl enable carnelian
sudo systemctl start carnelian

# Check status
sudo systemctl status carnelian

# View logs
sudo journalctl -u carnelian -f
```

---

## Linux-Specific Troubleshooting

### Docker Group Membership

If `docker` commands require `sudo`:

```bash
# Add your user to the docker group
sudo usermod -aG docker $USER

# Apply immediately (or log out and back in)
newgrp docker
```

### Firewall Rules (ufw)

If using `ufw`, allow the required ports:

```bash
# Carnelian API
sudo ufw allow 18789/tcp

# PostgreSQL (only if external access needed — not recommended)
sudo ufw allow 5432/tcp

# Ollama (only if external access needed)
sudo ufw allow 11434/tcp

# Check status
sudo ufw status
```

### SELinux / AppArmor

If Docker containers fail to start on SELinux-enabled systems:

```bash
# Check SELinux status
getenforce

# Temporarily set to permissive (for debugging)
sudo setenforce 0

# For permanent fix, add Docker SELinux policy or use :z/:Z volume flags
```

On AppArmor systems (Ubuntu default):

```bash
# Check AppArmor status
sudo aa-status

# If Docker is confined, check for denied operations
sudo dmesg | grep apparmor
```

### NVIDIA Driver Mismatch

If `nvidia-smi` works on the host but not in Docker:

```bash
# Check host driver version
nvidia-smi --query-gpu=driver_version --format=csv,noheader

# Check Docker runtime configuration
cat /etc/docker/daemon.json

# Reconfigure if needed
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker
```

### Port Conflicts

```bash
# Find what's using a port
sudo ss -tlnp | grep 18789

# Use a different port
cargo run --bin carnelian -- start --port 18790
```

---

## Headless Server Note

The Carnelian desktop UI (`carnelian-ui`) requires a display server (X11 or Wayland). On headless servers:

- **Skip the UI** — Run only the core orchestrator: `cargo run --bin carnelian -- start`. The REST API and WebSocket endpoints are fully functional without the UI.
- **Remote access** — Use the API endpoints from a remote machine or a web-based dashboard.
- **X forwarding** — If you need the UI on a headless server, use SSH X forwarding:

```bash
ssh -X user@server
export DISPLAY=:0
cargo run --bin carnelian-ui
```

- **VNC/RDP** — Alternatively, install a VNC server for remote desktop access.

For server deployments, the recommended approach is to run the orchestrator headless and interact via the REST API.

---

## Performance Tips

1. **Use GPU acceleration** — Ollama with GPU is 10–50x faster than CPU-only. Ensure NVIDIA Container Toolkit is properly configured.
2. **Allocate swap** — At least 4GB swap for large model loading: `sudo fallocate -l 4G /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile`.
3. **Parallel builds** — `cargo build --jobs $(nproc)` to use all CPU cores.
4. **tmpfs for target** — Mount `target/` on tmpfs for faster builds (if you have sufficient RAM): `mount -t tmpfs -o size=8G tmpfs target/`.
5. **IO scheduler** — Use `mq-deadline` or `none` for NVMe drives: `echo none | sudo tee /sys/block/nvme0n1/queue/scheduler`.

# 🔥 Carnelian OS — Windows Setup Guide (WSL2)

Carnelian runs inside WSL2 on Windows. This guide covers the full setup from scratch.

---

## Prerequisites Checklist

| Requirement | Version | Notes |
|-------------|---------|-------|
| Windows 10 (21H2+) or Windows 11 | — | WSL2 support required |
| WSL2 | Latest | Ubuntu 22.04 recommended |
| NVIDIA GPU drivers | Latest Game Ready or Studio | Installed on Windows host |
| Docker Desktop | Latest | WSL2 backend enabled |
| Rust | 1.85+ | Installed inside WSL2 |
| Node.js | 18+ | For Node.js worker |
| Python | 3.10+ | For Python worker |

---

## Step-by-Step WSL2 Setup

### 1. Enable WSL2

Open **PowerShell as Administrator**:

```powershell
# Install WSL with Ubuntu (default)
wsl --install

# Ensure WSL2 is the default version
wsl --set-default-version 2

# Restart your computer when prompted
```

After reboot, Ubuntu will launch automatically to complete setup. Create a Unix username and password.

### 2. Update Ubuntu

Inside the WSL2 terminal:

```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y build-essential pkg-config libssl-dev curl git
```

### 3. Install NVIDIA GPU Drivers (Windows Host)

Download and install the latest drivers from [nvidia.com/drivers](https://www.nvidia.com/drivers).

> **Important:** Install drivers on the *Windows host*, not inside WSL2. WSL2 automatically uses the host GPU drivers.

### 4. Verify GPU Passthrough

Inside WSL2:

```bash
# Should show your GPU
nvidia-smi
```

If `nvidia-smi` is not found, ensure you have the latest Windows NVIDIA drivers and WSL2 is up to date:

```powershell
# In PowerShell
wsl --update
```

### 5. Install Docker Desktop

1. Download from [docker.com/products/docker-desktop](https://www.docker.com/products/docker-desktop)
2. During installation, ensure **"Use WSL 2 instead of Hyper-V"** is checked
3. After installation, open Docker Desktop → **Settings**:
   - **General** → Enable "Use the WSL 2 based engine"
   - **Resources → WSL Integration** → Enable integration with your Ubuntu distro
   - **Resources → WSL Integration** → Enable GPU support (if available)

### 6. Verify Docker GPU Support

Inside WSL2:

```bash
# Basic Docker check
docker --version
docker-compose --version

# GPU passthrough test
docker run --rm --gpus all nvidia/cuda:12.0-base nvidia-smi
```

### 7. Install Rust

Inside WSL2:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify
rustc --version
cargo --version
```

### 8. Install Node.js and Python

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

---

## Carnelian First Run

All commands below run inside WSL2.

```bash
# 1. Clone repository
git clone https://github.com/kordspace/carnelian.git
cd carnelian

# 2. Build the project
cargo build --release

# 3. Run the interactive setup wizard
# This detects your hardware, sets up Docker containers, creates the database,
# generates your owner keypair, and activates starter skills.
carnelian init

# 4. Start the system
carnelian start
```

> **Non-interactive (CI/scripted):** `carnelian init --non-interactive`
> 
> For automated deployments, use the `--non-interactive` flag to skip all prompts.
> See [INSTALL.md](INSTALL.md) for detailed CI/CD setup options.

### Post-Init Setup (WSL2 Specific)

After running `carnelian init`, your system is ready. The wizard automatically:
- Starts PostgreSQL and Ollama Docker containers
- Runs database migrations
- Pulls appropriate models for your GPU profile
- Generates and secures your owner keypair

Access the dashboard at: http://localhost:18789

### Windows Host Access

To access Carnelian from Windows (outside WSL2):

```powershell
# In PowerShell (Windows side)
# Find WSL2 IP
wsl hostname -I
# Then use that IP:port in your browser
```

Or configure port forwarding in Windows Defender Firewall for port 18789.

---

## Windows-Specific Troubleshooting

### WSL2 Memory Limits

By default, WSL2 can consume up to 50% of host RAM. Create or edit `%USERPROFILE%\.wslconfig`:

```ini
[wsl2]
memory=16GB
processors=8
swap=4GB
```

Then restart WSL2:

```powershell
wsl --shutdown
```

### Port Forwarding

WSL2 ports are automatically forwarded to `localhost` on Windows. If ports aren't accessible:

```powershell
# Check WSL2 IP
wsl hostname -I

# Manual port forward (if needed)
netsh interface portproxy add v4tov4 listenport=18789 listenaddress=0.0.0.0 connectport=18789 connectaddress=$(wsl hostname -I)
```

### Docker Desktop WSL Integration

If Docker commands fail inside WSL2:

1. Open Docker Desktop → **Settings → Resources → WSL Integration**
2. Ensure your Ubuntu distro is toggled **on**
3. Restart Docker Desktop
4. In WSL2: `docker ps` should work without `sudo`

### File System Performance

> **Critical:** Store the Carnelian repository inside the WSL2 filesystem (`~/carnelian`), **not** on the Windows mount (`/mnt/c/...`). File operations on `/mnt/c/` are 5–10x slower due to the 9P filesystem bridge.

```bash
# Good (fast)
cd ~
git clone https://github.com/kordspace/carnelian.git

# Bad (slow)
cd /mnt/c/Users/marco/Documents
git clone https://github.com/kordspace/carnelian.git
```

### Firewall

If external access to Carnelian is needed, allow the port through Windows Firewall:

```powershell
# PowerShell as Administrator
New-NetFirewallRule -DisplayName "Carnelian" -Direction Inbound -LocalPort 18789 -Protocol TCP -Action Allow
```

---

## Performance Tips

1. **Store repo in WSL2 filesystem** — Not `/mnt/c/`. This is the single biggest performance improvement.
2. **Allocate sufficient memory** — Set `memory=16GB` or more in `.wslconfig` for Ollama + PostgreSQL + Rust builds.
3. **Use `cargo build --jobs N`** — Match to your CPU core count for faster builds.
4. **Docker resource limits** — Docker Desktop shares WSL2 resources. Ensure enough headroom for both Docker services and Rust compilation.
5. **VS Code Remote - WSL** — Use the VS Code WSL extension to edit files directly in the WSL2 filesystem.

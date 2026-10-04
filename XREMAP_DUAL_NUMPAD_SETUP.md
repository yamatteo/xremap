# Dual Numpad Split Keyboard Setup with xremap

This guide explains how to set up `xremap` from scratch on a new Linux machine (e.g. Ubuntu / Debian / Fedora with GNOME) to merge two identical USB numpads into a single unified split keyboard running as a systemd user service.

---

## 1. Install `xremap` Binary

You can either download a pre-built binary or build from source. Building from source is useful if you want the latest unreleased changes or a variant not covered by the published packages.

### Option A: Download Pre-Built Binary

Pre-built binaries are published for each desktop environment on GitHub.

1. Go to the [xremap GitHub Releases](https://github.com/xremap/xremap/releases) page (or download via `curl`/`wget`).
2. Download the package matching your desktop environment (e.g., GNOME on x86_64):
   ```bash
   # Download the latest release binary for GNOME
   curl -s https://api.github.com/repos/xremap/xremap/releases/latest \
     | grep "browser_download_url.*linux-x86_64-gnome.zip" \
     | cut -d '"' -f 4 \
     | wget -qi - -O xremap-gnome.zip

   # Unzip and install to /usr/local/bin
   unzip xremap-gnome.zip
   sudo install -m 755 xremap /usr/local/bin/xremap
   rm xremap xremap-gnome.zip
   ```

### Option B: Build from Source

1. **Install Rust** (via [rustup](https://rustup.rs/)):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source "$HOME/.cargo/env"
   ```

2. **Install build dependencies** (Ubuntu/Debian):
   ```bash
   sudo apt install build-essential libx11-dev
   ```

3. **Clone the repository and build** with the feature matching your desktop environment (GNOME shown here). The branch below carries the dual numpad example and the operator features it uses (layer-tap, and `device:`/`mode:` filters on `experimental_map`); the systemd unit expects the checkout at `~/xremap`:
   ```bash
   git clone -b feat/tap-hold-next-release https://github.com/yamatteo/xremap.git ~/xremap
   cd ~/xremap
   cargo build --release --features gnome
   ```

4. **Install the binary:**
   ```bash
   sudo install -m 755 target/release/xremap /usr/local/bin/xremap
   ```

### Verify the Installation

```bash
xremap --version
```

---

## 2. Configure Input & `uinput` Permissions (Run without sudo)

To allow your user account to read physical input events and emit virtual key events without root privileges:

1. **Add your user to the `input` group:**
   ```bash
   sudo usermod -aG input $USER
   ```

2. **Configure `/dev/uinput` permissions:**
   ```bash
   echo 'KERNEL=="uinput", GROUP="input", TAG+="uaccess", MODE:="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-input.rules
   ```

3. **Ensure the `uinput` kernel module loads on boot:**
   ```bash
   echo uinput | sudo tee /etc/modules-load.d/uinput.conf
   sudo modprobe uinput
   ```

4. **Apply permissions immediately:**
   ```bash
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

> **Important:** You must **log out and log back in** (or reboot) for your user's new `input` group membership to take effect.

---

## 3. Set Up Persistent Device Symlinks for Identical Numpads

Because identical numpads share the same Vendor ID, Product ID, and device names, they must be distinguished by the physical USB port they are plugged into.

### A. Identify USB Port Paths
Run `udevadm info` on your connected event devices or check the existing path names:
```bash
ls -l /dev/input/by-path/
```
Find the USB bus/port identifier for each numpad (e.g. `0:3.2` for Left and `0:3.3` for Right).

### B. Create custom udev rules
Create `/etc/udev/rules.d/99-numpads.rules`:
```bash
sudo tee /etc/udev/rules.d/99-numpads.rules << 'EOF'
# Left Numpad (Port 3.2)
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_PATH}=="*0:3.2:1.0*", ATTRS{name}=="SIGMACHIP USB Keyboard", SYMLINK+="input/by-id/numpad-left-kbd"
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_PATH}=="*0:3.2:1.1*", ATTRS{name}=="SIGMACHIP USB Keyboard Consumer Control", SYMLINK+="input/by-id/numpad-left-consumer"

# Right Numpad (Port 3.3)
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_PATH}=="*0:3.3:1.0*", ATTRS{name}=="SIGMACHIP USB Keyboard", SYMLINK+="input/by-id/numpad-right-kbd"
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_PATH}=="*0:3.3:1.1*", ATTRS{name}=="SIGMACHIP USB Keyboard Consumer Control", SYMLINK+="input/by-id/numpad-right-consumer"
EOF
```

### C. Reload and Trigger udev
```bash
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=input
```

Verify that the persistent symlinks now exist:
```bash
ls -l /dev/input/by-id/numpad-*
```

---

## 4. Get the `xremap` Configuration Files

The configuration and the systemd unit live in this repository, under
[`example/dual_numpad/`](example/dual_numpad/):

| File | Purpose |
| --- | --- |
| `config.yml` | Maps the raw numpad keycodes of each pad to letters, and handles the homerow tap-holds and the move and numbers modes. |
| `xremap.service` | Runs xremap with `config.yml`. |

`config.yml` has three parts, applied in this order:

- `experimental_map`: tap-hold on the homerow. R S T and N E I are modifiers when held;
  A and O are layer-taps, held for numbers on the opposite pad. Each entry is limited to
  one pad with `device:`, and to the modes where its key is still a letter with `mode:`.
- `modmap`: per-pad keycode to letter mapping, and the move and numbers modes.
- `keymap`: the left numbers mode, which needs Shift combos.

The unit reads the config straight from the checkout and expects it at `~/xremap`. If you
did not already clone it while building from source:

```bash
git clone -b feat/tap-hold-next-release https://github.com/yamatteo/xremap.git ~/xremap
```

If you keep the checkout elsewhere, adjust the `ExecStart=` path in the unit file.

---

## 5. Set Up Systemd User Service

1. **Create the user systemd directory:**
   ```bash
   mkdir -p ~/.config/systemd/user
   ```

2. **Link `xremap.service` into it:**
   ```bash
   ln -s ~/xremap/example/dual_numpad/xremap.service ~/.config/systemd/user/
   ```

   The unit is:
   ```ini
   [Unit]
   Description=xremap key remapper

   [Service]
   ExecStart=/usr/local/bin/xremap --watch=config,device --device "SIGMACHIP" %h/xremap/example/dual_numpad/config.yml
   Restart=always
   RestartSec=3
   StandardOutput=journal
   StandardError=journal

   [Install]
   WantedBy=default.target
   ```

   `--watch=config,device` reloads `config.yml` when it is edited and picks up the numpads
   when they are plugged in later.

3. **Enable and start the service:**
   ```bash
   systemctl --user daemon-reload
   systemctl --user enable --now xremap.service
   ```

---

## 6. Upgrading from the Two-Service Setup

Earlier versions of this setup ran the homerow tap-holds in a second instance,
`xremap-layers.service`, reading `layers.yml`. Both are now part of `config.yml`. If you
had it installed, remove it, since it would sit idle waiting for a device that no longer
exists. Rebuild and reinstall the binary first (Option B in section 1): older builds reject
the `device:` and `mode:` fields that `config.yml` now uses in `experimental_map`.

```bash
systemctl --user disable --now xremap-layers.service
rm ~/.config/systemd/user/xremap-layers.service
systemctl --user daemon-reload
systemctl --user restart xremap.service
```

---

## 7. Management and Verification

- **Check Service Status:**
  ```bash
  systemctl --user status xremap.service
  ```

- **Inspect Live Logs:**
  ```bash
  journalctl --user -fu xremap.service
  ```

- **After Editing a Config:** nothing to do, `--watch=config,device` reloads it on save.

- **Restart After Editing a Unit File or Reinstalling the Binary:**
  ```bash
  systemctl --user daemon-reload
  systemctl --user restart xremap.service
  ```

- **Test Manually in Terminal (Debugging):** stop the service first, then run:
  ```bash
  systemctl --user stop xremap.service
  xremap --watch=config,device --device "SIGMACHIP" ~/xremap/example/dual_numpad/config.yml
  ```

  Mode changes are printed as `mode: <name>`.

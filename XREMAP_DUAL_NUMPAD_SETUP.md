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

3. **Clone the repository and build** with the feature matching your desktop environment (GNOME shown here):
   ```bash
   git clone https://github.com/xremap/xremap.git
   cd xremap
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

## 4. Create the `xremap` Configuration File

Create the configuration directory and file:
```bash
mkdir -p ~/.config/xremap
```

Create `~/.config/xremap/config.yml`:

```yaml
modmap:
  - name: base_left
    device:
      only:
        - /dev/input/by-id/numpad-left-kbd
        - /dev/input/by-id/numpad-left-consumer
    remap:
      KP0: TAB
      KP1: Q
      KP4: W
      KP7: F
      NUMLOCK: P
      HOMEPAGE: G

      SPACE: BACKSPACE
      KP2: A
      KP5: R
      KP8: S
      KPSLASH: T
      TAB: D

      KPDOT: ESC
      KP3: Z
      KP6: X
      KP9: C
      KPASTERISK: V
      MAIL: B

      KPENTER: ESC
      KPPLUS: ESC
      KPMINUS: ESC
      BACKSPACE: ESC
      CALC: SPACE

  - name: base_right
    device:
      only:
        - /dev/input/by-id/numpad-right-kbd
        - /dev/input/by-id/numpad-right-consumer
    remap:
      CALC: J
      BACKSPACE: L
      KPMINUS: U
      KPPLUS: Y
      KPENTER: SEMICOLON

      MAIL: H
      KPASTERISK: N
      KP9: E
      KP6: I
      KP3: O
      KPDOT: DELETE

      TAB: K
      KPSLASH: M
      KP8: COMMA
      KP5: DOT
      KP2: SLASH
      SPACE: ESC

      HOMEPAGE: ENTER
      NUMLOCK: P
      KP7: F
      KP4: W
      KP1: Q
      KP0: TAB
```

---

## 5. Set Up Systemd User Service

1. **Create the user systemd directory:**
   ```bash
   mkdir -p ~/.config/systemd/user
   ```

2. **Create `~/.config/systemd/user/xremap.service`:**
   ```ini
   [Unit]
   Description=xremap dual numpad service
   After=default.target

   [Service]
   ExecStart=/usr/local/bin/xremap --watch --device "SIGMACHIP" %h/.config/xremap/config.yml
   Restart=always
   RestartSec=3
   StandardOutput=journal
   StandardError=journal

   [Install]
   WantedBy=default.target
   ```

3. **Enable and start the service:**
   ```bash
   systemctl --user daemon-reload
   systemctl --user enable --now xremap.service
   ```

---

## 6. Set Up the Layers Service (tap-hold homerow mods)

`layers.yml` applies a second remap pass (e.g. tap-hold homerow mods) on top of the merged
output produced by `xremap.service`. Since it needs to grab the virtual `xremap-numpads`
device, this service must start *after* the first one is already running and producing that
device.

1. **Create `~/.config/systemd/user/xremap-layers.service`:**
   ```ini
   [Unit]
   Description=xremap key remapper for numpad layers (tap-hold homerow mods)
   After=xremap.service
   BindsTo=xremap.service

   [Service]
   ExecStart=/usr/local/bin/xremap --watch --device "xremap-numpads" %h/.config/xremap/layers.yml
   Restart=always
   RestartSec=3
   StandardOutput=journal
   StandardError=journal

   [Install]
   WantedBy=default.target
   ```

   `After=` orders it behind `xremap.service`, and `BindsTo=` ties its lifecycle to it: if
   `xremap.service` stops or restarts (e.g. after editing `config.yml`), `xremap-layers.service`
   is stopped too, and will start back up once `xremap.service` is running again.

2. **Enable and start the service:**
   ```bash
   systemctl --user daemon-reload
   systemctl --user enable --now xremap-layers.service
   ```

   Enabling `xremap-layers.service` also starts `xremap.service` first, thanks to the
   `BindsTo=`/`After=` ordering above.

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

- **Restart Service After Editing Config:**
  ```bash
  systemctl --user restart xremap.service
  ```

- **Test Manually in Terminal (Debugging):**
  ```bash
  xremap --watch --device "SIGMACHIP" ~/.config/xremap/config.yml
  ```

---
name: setup-cuda
description: Install NVIDIA CUDA Toolkit on Linux (Ubuntu/Debian, RHEL/Fedora)
platform: linux
last_reviewed: 2026-03-11
author: noah-team
source: bundled
emoji: 🎮
---

# Set Up NVIDIA CUDA Toolkit

## When to activate
User wants to install CUDA, set up GPU computing, needs NVIDIA drivers for deep learning/ML, or mentions nvcc/nvidia-smi not found on Linux.

## Quick check
Run `shell_run` with `lspci | grep -i nvidia` to verify an NVIDIA GPU is present.
- If no NVIDIA GPU found → wrong playbook, tell user CUDA requires an NVIDIA GPU.
- If GPU found → continue.

## Step 1: Check existing installation
Run `shell_run` with `nvidia-smi` and `nvcc --version`.
- If both work and versions are satisfactory → skip to Step 6 (verify).
- If `nvidia-smi` works but `nvcc` missing → skip to Step 4 (toolkit only).
- If neither works → continue with Step 2.

## Step 2: Detect distro and install prerequisites
Run `shell_run` with `cat /etc/os-release` to identify the distribution.
Then install kernel headers and GCC:

**Ubuntu/Debian:**
```
sudo apt update && sudo apt install -y build-essential linux-headers-$(uname -r)
```

**RHEL/Fedora/Rocky:**
```
sudo dnf install -y gcc kernel-devel-$(uname -r)
```

## Step 3: Add NVIDIA repository and install driver
**Ubuntu/Debian:**
```
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2404/x86_64/cuda-keyring_1.1-1_all.deb
sudo dpkg -i cuda-keyring_1.1-1_all.deb
sudo apt update
```
Adjust `ubuntu2404` to match the actual distro version (ubuntu2204, debian12, etc.).

**RHEL/Fedora/Rocky:**
```
sudo dnf config-manager --add-repo https://developer.download.nvidia.com/compute/cuda/repos/rhel9/x86_64/cuda-rhel9.repo
```
Adjust `rhel9` to match (fedora41, etc.).

Use WAIT_FOR_USER — adding repos and downloading packages can take a few minutes.

## Step 4: Install CUDA Toolkit
**Ubuntu/Debian:**
```
sudo apt install -y cuda-toolkit
```

**RHEL/Fedora/Rocky:**
```
sudo dnf install -y cuda-toolkit
```

This installs the compiler (nvcc), libraries, and headers. The driver is pulled in as a dependency if not already installed.

Use WAIT_FOR_USER — installation downloads ~2–4 GB and takes 5–15 minutes.

## Step 5: Configure PATH
Add CUDA to the user's PATH by appending to `~/.bashrc`:
```
echo 'export PATH=/usr/local/cuda/bin${PATH:+:${PATH}}' >> ~/.bashrc
echo 'export LD_LIBRARY_PATH=/usr/local/cuda/lib64${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}' >> ~/.bashrc
source ~/.bashrc
```

Tell the user to reboot if a new NVIDIA driver was installed:
```
sudo reboot
```

## Step 6: Verify installation
Run these checks:
```
nvidia-smi
nvcc --version
```
- `nvidia-smi` should show the GPU model and driver version.
- `nvcc` should show the CUDA compiler version.

> This sequence resolves ~90% of CUDA setup issues on supported Linux distributions.

## Caveats
- If the system uses **Secure Boot**, the NVIDIA kernel module may fail to load. The user needs to either disable Secure Boot in BIOS or enroll a MOK signing key during installation.
- If **nouveau** (open-source NVIDIA driver) is loaded, it must be blacklisted first: `echo 'blacklist nouveau' | sudo tee /etc/modprobe.d/blacklist-nouveau.conf && sudo update-initramfs -u` (Ubuntu) or `sudo dracut --force` (RHEL).
- On **Fedora 41+** with GCC version mismatches, install the compatibility GCC package and set `NVCC_CCBIN` accordingly.

## Tools referenced
- `shell_run` — run commands to check GPU, install packages, configure PATH
- `ui_spa` with WAIT_FOR_USER — for long-running installs and reboot steps
- `ui_user_question` — ask which distro if auto-detection fails

## Escalation
If the GPU is too old (compute capability < 5.0), CUDA 12+ won't support it — suggest an older CUDA version. If driver installation causes black screen/boot failure, advise booting to recovery mode and running `sudo apt remove --purge 'nvidia-*'` or equivalent.

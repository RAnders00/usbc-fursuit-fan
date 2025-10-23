#!/usr/bin/env python3
import subprocess
import sys
import re

# === CONFIGURATION ===
BINARY_NAME = "fursuit-fan-controller-fw"
TARGET = "thumbv7m-none-eabihf"
BUILD_TYPE = "release"  # or "debug"
FLASH_SIZE = 64 * 1024  # in bytes
SRAM_SIZE = 20 * 1024    # in bytes

# === Run cargo objdump ===
elf_path = f"target/{TARGET}/{BUILD_TYPE}/{BINARY_NAME}"
try:
    output = subprocess.check_output(
        ["cargo", "objdump", "--bin", BINARY_NAME, f"--{BUILD_TYPE}", "--", "-h"],
        universal_newlines=True
    )
except subprocess.CalledProcessError as e:
    print("Failed to run cargo objdump. Is the project built?", file=sys.stderr)
    sys.exit(1)

# === Parse section sizes ===
flash_sections = {'.text', '.rodata', '.data', '.flash_filesystem'}
sram_sections = {'.data', '.bss'}

flash_used = 0
sram_used = 0

section_pattern = re.compile(r'^\s*\d+\s+(\S+)\s+([0-9a-fA-F]+)')

for line in output.splitlines():
    match = section_pattern.match(line)
    if match:
        section = match.group(1)
        size = int(match.group(2), 16)
        if section in flash_sections:
            flash_used += size
        elif section in sram_sections:
            sram_used += size
        else:
            print("Ignoring section: {section}", section)

# === Report ===
def percent(part, total):
    return f"{(100.0 * part / total):.2f}%"

print(f"\nMemory Usage for {BINARY_NAME} ({BUILD_TYPE}):\n")
print(f"Flash: {flash_used} / {FLASH_SIZE} bytes ({percent(flash_used, FLASH_SIZE)})")
print(f"SRAM : {sram_used} / {SRAM_SIZE} bytes ({percent(sram_used, SRAM_SIZE)})")

flash_remaining = FLASH_SIZE - flash_used
flash_pages_probably_remaining = int(flash_remaining / 1024)
if flash_pages_probably_remaining > 0:
    print(f"\nThe filesystem could probably be expanded by {flash_pages_probably_remaining} more pages.")

#!/usr/bin/env python3
"""
measure_cpu.py — build each program at multiple font sizes and measure peak CPU.

For each font size, runs `make clean && make FONT_SIZE=N`, then times each
program and prints a Markdown table of peak CPU percentages.
"""

import subprocess, time, os, sys

DURATION  = 4
INTERVAL  = 1.0
CLK_TCK   = os.sysconf(os.sysconf_names['SC_CLK_TCK'])
FONT_SIZES = [12, 24, 36, 64]

PROGRAMS = [
    ("shapes", ["./shapes"]),
    ("chars",  ["./chars"]),
    ("mono",   ["./mono",  "RobotoMono-Regular.font"]),
    ("tt",     ["./tt",    "Roboto-Regular.font"]),
]

def read_stat(pid):
    with open(f"/proc/{pid}/stat") as f:
        fields = f.read().split()
    return int(fields[13]) + int(fields[14])

def measure(cmd):
    proc = subprocess.Popen(cmd, stderr=subprocess.DEVNULL)
    pid  = proc.pid
    peak = 0.0
    time.sleep(0.2)
    prev_ticks = read_stat(pid)
    prev_time  = time.monotonic()
    deadline   = time.monotonic() + DURATION
    while time.monotonic() < deadline:
        time.sleep(INTERVAL)
        try:
            cur_ticks = read_stat(pid)
        except FileNotFoundError:
            break
        cur_time = time.monotonic()
        cpu_pct  = (cur_ticks - prev_ticks) / CLK_TCK / (cur_time - prev_time) * 100.0
        if cpu_pct > peak:
            peak = cpu_pct
        prev_ticks = cur_ticks
        prev_time  = cur_time
    proc.terminate()
    proc.wait()
    return peak

def build(font_size):
    print(f"  building FONT_SIZE={font_size}...", flush=True)
    subprocess.run(["make", "clean"], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    subprocess.run(["make", f"FONT_SIZE={font_size}"], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

# results[label][font_size] = peak%
results = {label: {} for label, _ in PROGRAMS}

for size in FONT_SIZES:
    print(f"\nFont size {size}pt:", flush=True)
    build(size)
    for label, cmd in PROGRAMS:
        print(f"  measuring {label}...", flush=True)
        peak = measure(cmd)
        results[label][size] = peak

# Print Markdown table
col_w = 8
header = f"| {'Program':<10} |" + "".join(f" {str(s)+'pt':>{col_w}} |" for s in FONT_SIZES)
sep    = f"| {'-'*10} |" + "".join(f" {'-'*col_w} |" for s in FONT_SIZES)
print(f"\n{header}")
print(sep)
for label, _ in PROGRAMS:
    row = f"| {label:<10} |"
    for size in FONT_SIZES:
        cell = f"{results[label][size]:.1f}%"
        row += f" {cell:>{col_w}} |"
    print(row)

#!/usr/bin/env python3
"""
measure_cpu.py — run each program for DURATION seconds, report peak CPU usage.
Usage: python3 measure_cpu.py

Resolution: at CLK_TCK=100 and INTERVAL=1.0, smallest measurable step is
1 tick / 100 ticks/sec / 1 sec * 100 = 1%.  Reduce INTERVAL to trade
resolution for responsiveness, but don't go below ~0.5s.
"""

import subprocess, time, os

DURATION  = 4    # seconds to run each program
INTERVAL  = 1.0   # sampling interval — 1s gives ~1% resolution at CLK_TCK=100
CLK_TCK   = os.sysconf(os.sysconf_names['SC_CLK_TCK'])
OUTPUT    = "cpu_results.txt"

PROGRAMS = [
    ("shapes", ["./shapes"]),
    ("chars",  ["./chars"]),
    ("mono",   ["./mono",  "Roboto-Regular.font"]),
    ("tt",     ["./tt",    "Roboto-Regular.font"]),
]

def read_stat(pid):
    """Return utime+stime in ticks from /proc/pid/stat."""
    with open(f"/proc/{pid}/stat") as f:
        fields = f.read().split()
    return int(fields[13]) + int(fields[14])

def measure(label, cmd):
    proc = subprocess.Popen(cmd)
    pid  = proc.pid
    peak = 0.0

    time.sleep(0.2)  # brief pause for process to start

    prev_ticks = read_stat(pid)
    prev_time  = time.monotonic()

    deadline = time.monotonic() + DURATION
    while time.monotonic() < deadline:
        time.sleep(INTERVAL)
        try:
            cur_ticks = read_stat(pid)
        except FileNotFoundError:
            break

        cur_time   = time.monotonic()
        cpu_pct    = (cur_ticks - prev_ticks) / CLK_TCK / (cur_time - prev_time) * 100.0

        if cpu_pct > peak:
            peak = cpu_pct

        prev_ticks = cur_ticks
        prev_time  = cur_time

    proc.terminate()
    proc.wait()

    result = f"{label:<20} peak CPU: {peak:.1f}%"
    print(result)
    return result

results = []
for label, cmd in PROGRAMS:
    results.append(measure(label, cmd))

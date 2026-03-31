#!/bin/bash
# Regenerate demo app ui.rs from demo.lua
cd /home/jk/projects/SR835_firmware/imgrids
lua scripts/layout.lua < examples/demo.lua > examples/app/ui.rs

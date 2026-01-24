#! /usr/bin/env sh

set -x

mkdir roms
echo "Fetching CGB bootrom"
wget -O roms/cgb_boot.bin https://gbdev.gg8.se/files/roms/bootroms/cgb_boot.bin

sudo apt install libasound2-dev

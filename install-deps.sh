#! /usr/bin/env sh

set -x

mkdir roms
cd roms
echo "Fetching CGB bootrom"
wget https://gbdev.gg8.se/files/roms/bootroms/cgb_boot.bin
cd ..

sudo apt install libasound2-dev

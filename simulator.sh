#!/bin/bash
set -o pipefail

# This script is used to launch the qemu tool with specific arguments
# and pipe its output to the defmt-print tool for further processing.

# Check if qemu tool exists in PATH
if ! command -v qemu-system-riscv32 &> /dev/null; then
    echo "qemu-system-riscv32 tool not found in PATH"
    exit 1
fi

# Path to the qemu tool
QEMU_PATH="qemu-system-riscv32"

# Check if defmt-print tool exists in PATH
if ! command -v defmt-print &> /dev/null; then
    echo "defmt-print tool not found in PATH"
    exit 1
fi

# Path to the defmt-print tool
DEFMT_PRINT_PATH="defmt-print"

# Arguments for the qemu tool from config.toml
CONFIG_ARGS="-M virt -bios none -semihosting -cpu rv32,a=true,\
c=false,f=true,d=true,zba=true,zbb=true,zbs=true,zcf=true,zifencei=true,\
zicsr=true,zihintpause=true,zicond=true,zicntr=true,zfa=true,zfh=true,\
zca=true,zcb=true,zcmp=true,zcmt=true,zbc=true   -nographic -kernel "

# Launch qemu tool with arguments from config.toml and additional arguments
# Pipe the output to defmt-print for further processing
if [[ " $@ " == *" -gdb "* ]]; then
    "$QEMU_PATH" $CONFIG_ARGS "${@:1:$#-1}" | "$DEFMT_PRINT_PATH" -e "${@: -1}"
else
    "$QEMU_PATH" $CONFIG_ARGS "$@" | "$DEFMT_PRINT_PATH" -e "${@: -1}"
fi

exit "$?"

# Description of defmt-print:
# defmt-print is a tool used to decode and print log messages
# produced by embedded systems using the defmt logging framework.
# It takes the encoded log messages from the qemu tool and
# translates them into a human-readable format.

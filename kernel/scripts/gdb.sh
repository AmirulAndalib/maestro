#!/bin/sh

# This script allows to run gdb to debug the kernel using QEMU.

# Environment variables:
# - ARCH: specifies the architecture to build for
# - AUX_ELF: specifies the path to an auxiliary ELF file whose symbols will be added to gdb

if [ -z "$ARCH" ]; then
  ARCH="x86_64"
fi

cargo build $CARGOFLAGS --target arch/$ARCH/$ARCH.json

export QEMUFLAGS="$QEMUFLAGS -s -S -d int"
setsid cargo run $CARGOFLAGS --target arch/$ARCH/$ARCH.json >qemu.log 2>&1 &
QEMU_PID=$!

KERN_PATH="target/$ARCH/debug/maestro"

if ! [ -z "$AUX_ELF" ]; then
	gdb $KERN_PATH -ex 'target remote :1234' -ex 'set confirm off' -ex 'add-symbol-file -o $AUX_ELF' -ex 'set confirm on'
else
	gdb $KERN_PATH -ex 'target remote :1234'
fi

kill -- -$QEMU_PID

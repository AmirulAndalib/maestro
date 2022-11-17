# This is the main makefile for the kernel's compilation
#
# The kernel is divided into two parts:
# - Rust code, which represents most of the kernel
# - Assembly and C code
#
# The compilation occurs in the following order:
# - The makefile compiles assembly and C code and pack them into static a library
# - The Rust code is compiled using Cargo
# - Cargo calls the build script, which tells the Rust compiler to link the library previously
# mentioned
# - Cargo runs the linker with the linker script for the required target
#
# This makefile also contains several rules used to test the kernel with emulators



# The name of the kernel image
NAME = maestro

# Current directory
PWD := $(shell pwd)
# The list of files that, when touched, make the kernel be recompiled entirely.
TOUCH_UPDATE_FILES = Makefile



# ------------------------------------------------------------------------------
#    Configuration variables
# ------------------------------------------------------------------------------



# The path to the configuration file created by the configuration utility
CONFIG_FILE = .config
# Tells whether the configuration file exists
CONFIG_EXISTS = $(shell stat $(CONFIG_FILE) >/dev/null 2>&1; echo $$?)

# The path to the script that generates configuration as compiler arguments
CONFIG_ARGS_SCRIPT = scripts/config_args.sh
# The path to the script that generates configuration as environment variables
CONFIG_ENV_SCRIPT = scripts/config_env.sh
# The path to the script that extracts specific configuration attributes
CONFIG_ATTR_SCRIPT = scripts/config_attr.sh

# Configuration as arguments for the compiler
CONFIG_ARGS := $(shell $(CONFIG_ARGS_SCRIPT))
# Configuration as environment variables
CONFIG_ENV := $(shell $(CONFIG_ENV_SCRIPT))

# The target architecture
CONFIG_ARCH := $(shell $(CONFIG_ATTR_SCRIPT) GENERAL_ARCH)
# Tells whether to compile in debug mode
CONFIG_DEBUG := $(shell $(CONFIG_ATTR_SCRIPT) DEBUG_DEBUG)
# Tells whether to compile for unit testing
CONFIG_DEBUG_TEST := $(shell $(CONFIG_ATTR_SCRIPT) DEBUG_TEST)



# ------------------------------------------------------------------------------
#    Kernel compilation variables
# ------------------------------------------------------------------------------



# The path to the architecture specific directory
ARCH_PATH = $(PWD)/arch/$(CONFIG_ARCH)/

# The target descriptor file path
TARGET = $(ARCH_PATH)target.json
# The linker script file path
LINKER = $(ARCH_PATH)linker.ld

# The directory containing sources
SRC_DIR = $(PWD)/src/
# The directory containing object files to link
OBJ_DIR = $(PWD)/obj/

# The name of the library containg the C/Assembly code.
LIB_NAME = lib$(NAME).a

# The C language compiler
ifeq ($(CONFIG_ARCH),x86)
CC = i686-elf-gcc
else
CC = $(CONFIG_ARCH)-elf-gcc
endif

# The C language compiler flags
CFLAGS = -nostdlib -ffreestanding -fno-stack-protector -fno-pic -mno-red-zone -Wall -Wextra -Werror -lgcc
ifeq ($(CONFIG_DEBUG), false)
CFLAGS += -O3
else
CFLAGS += -g3
endif

# The archive creator program
AR = i686-elf-ar
# The archive creator program flags
ARFLAGS = rc

# The list of assembly source files
ASM_SRC := $(shell find $(SRC_DIR) -type f -name "*.s" -and ! -name "crti.s" -and ! -name "crtn.s")
# The list of C language source files
C_SRC := $(shell find $(SRC_DIR) -type f -name "*.c")
# The list of C language header files
HDR := $(shell find $(SRC_DIR) -type f -name "*.h")

# The list of directories in the source directory
DIRS := $(shell find $(SRC_DIR) -type d)
# The list of object directories
OBJ_DIRS := $(patsubst $(SRC_DIR)%, $(OBJ_DIR)%, $(DIRS))

# The list of assembly objects
ASM_OBJ := $(patsubst $(SRC_DIR)%.s, $(OBJ_DIR)%.s.o, $(ASM_SRC))
# The list of C language objects
C_OBJ := $(patsubst $(SRC_DIR)%.c, $(OBJ_DIR)%.c.o, $(C_SRC))

# The list of objects
OBJ := $(ASM_OBJ) $(C_OBJ)

# Cargo
CARGO = cargo +nightly
# Cargo flags
CARGOFLAGS = --verbose -Zbuild-std=core --target $(TARGET)
ifeq ($(CONFIG_DEBUG), false)
CARGOFLAGS += --release
endif
ifeq ($(CONFIG_DEBUG_TEST), true)
CARGOFLAGS += --tests
endif

# The Rust language compiler flags
RUSTFLAGS = -Zmacro-backtrace $(CONFIG_ARGS)
ifeq ($(CONFIG_DEBUG), true)
RUSTFLAGS += -Cforce-frame-pointers=y -Cdebuginfo=2
endif

# The list of Rust language source files
RUST_SRC := $(shell find $(SRC_DIR) -type f -name "*.rs")

# The list of all sources to compile
SRC := $(ASM_SRC) $(C_SRC) $(RUST_SRC)



# ------------------------------------------------------------------------------
#    Checking for configuration file & documentation compilation
# ------------------------------------------------------------------------------



# The path to the documentation sources
DOC_SRC_DIR = doc/src/
# The list of documentation sources
DOC_SRC = $(shell find $(DOC_SRC_DIR) -type f)
# The path to the documentation build directory
DOC_DIR = doc/book/



ifeq ($(CONFIG_EXISTS), 0)
# The rule to compile everything
 ifeq ($(CONFIG_DEBUG_TEST), false)
all: $(NAME) iso doc
 else
all: $(NAME) iso
 endif

# Builds the documentation
doc: $(SRC) $(DOC_SRC)
	$(CONFIG_ENV) RUSTFLAGS='$(RUSTFLAGS)' $(CARGO) doc $(CARGOFLAGS) --document-private-items
	mdbook build doc/
	rm -rf $(DOC_DIR)/references/
	cp -r target/target/doc/ $(DOC_DIR)/references/
else
all:
	echo "File $(CONFIG_FILE) doesn't exist. Create it from file `default.config`"
	false

doc: all

.SILENT: all doc
endif

.PHONY: all doc



# ------------------------------------------------------------------------------
#    Kernel compilation
# ------------------------------------------------------------------------------



# The rule to create object directories
$(OBJ_DIRS):
	mkdir -p $(OBJ_DIRS)

# The rule to build the library
$(LIB_NAME): $(OBJ_DIRS) $(OBJ)
	$(AR) $(ARFLAGS) $@ $(OBJ)

# The rule to compile assembly objects
$(OBJ_DIR)%.s.o: $(SRC_DIR)%.s $(HDR) $(TOUCH_UPDATE_FILES)
	$(CC) $(CFLAGS) -I $(SRC_DIR) -c $< -o $@

# The rule to compile C language objects
$(OBJ_DIR)%.c.o: $(SRC_DIR)%.c $(HDR) $(TOUCH_UPDATE_FILES)
	$(CC) $(CFLAGS) -I $(SRC_DIR) -c $< -o $@

# The rule to compile the kernel image
$(NAME): $(LIB_NAME) $(RUST_SRC) $(LINKER) $(TOUCH_UPDATE_FILES)
	$(CONFIG_ENV) RUSTFLAGS='$(RUSTFLAGS)' $(CARGO) build $(CARGOFLAGS)
ifeq ($(CONFIG_DEBUG_TEST), false)
 ifeq ($(CONFIG_DEBUG), false)
	$(CC) $(CFLAGS) -o $(NAME) target/target/release/libkernel.a -T$(LINKER)
 else
	$(CC) $(CFLAGS) -o $(NAME) target/target/debug/libkernel.a -T$(LINKER)
 endif
else
	cp `find target/target/debug/deps/ -name 'kernel-*' -executable` maestro
endif

# Alias for $(NAME).iso
iso: $(NAME).iso

# The rule to compile the .iso file image, using grub as a bootloader
$(NAME).iso: $(NAME) grub.cfg
	mkdir -p iso/boot/grub
	cp $(NAME) iso/boot
	cp grub.cfg iso/boot/grub
	grub-mkrescue -o $(NAME).iso iso

# Runs clippy on the Rust code
clippy:
	$(CONFIG_ENV) RUSTFLAGS='$(RUSTFLAGS)' $(CARGO) clippy $(CARGOFLAGS)

# Updates the tags file
tags: $(SRC)
	ctags --languages=+rust $(SRC)

.PHONY: iso clippy



# ------------------------------------------------------------------------------
#    Emulators
# ------------------------------------------------------------------------------



# The QEMU disk file
QEMU_DISK = qemu_disk
# The size of the QEMU disk in megabytes
QEMU_DISK_SIZE = 1024
# Flags for the QEMU emulator
QEMU_FLAGS = -smp cpus=2 -cdrom $(NAME).iso -drive file=$(QEMU_DISK),format=raw \
	-device isa-debug-exit,iobase=0xf4,iosize=0x04

# If `1`, QEMU is run into the terminal
QEMU_TERM ?= 0

ifeq ($(QEMU_TERM), 1)
QEMU_FLAGS += -serial mon:stdio -nographic
else
QEMU_FLAGS += -serial file:serial.log
endif

# Creates the disk for the QEMU emulator
$(QEMU_DISK):
	dd if=/dev/zero of=$(QEMU_DISK) bs=1M count=$(QEMU_DISK_SIZE) status=progress

# Runs the kernel with QEMU
run: iso $(QEMU_DISK)
	qemu-system-i386 $(QEMU_FLAGS)

# The rule to test the kernel using QEMU
test: iso $(QEMU_DISK)
	qemu-system-i386 $(QEMU_FLAGS) -d int

# Rule used to debug the kernel using GDB
debug: $(NAME).iso $(QEMU_DISK)
	qemu-system-i386 $(QEMU_FLAGS) -s -S -d int >debug_out 2>&1

# The rule to run the kernel's selftests using QEMU
selftest: iso $(QEMU_DISK)
	qemu-system-i386 $(QEMU_FLAGS) -nographic >/dev/null

# The rule to run a CPU test of the kernel using QEMU (aka running the kernel and storing a lot of
# logs into the `cpu_out` file)
cputest: iso
	qemu-system-i386 $(QEMU_FLAGS) -d int,cpu >cpu_out 2>&1

.PHONY: test selftest cputest



# ------------------------------------------------------------------------------
#    Cleaning up
# ------------------------------------------------------------------------------



# The rule to clean the workspace
clean:
	rm -rf $(OBJ_DIR)
	rm -rf $(LIB_NAME)
	rm -rf iso/

# The rule to clean the workspace, including target binaries
fclean: clean
	rm -rf target/
	rm -f $(NAME)
	rm -f $(NAME).iso
	rm -rf $(DOC_DIR)

# The rule to recompile everything
re: fclean all

.PHONY: clean fclean re

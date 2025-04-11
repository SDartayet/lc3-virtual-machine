# LC3 Virtual Machine

## An implementation of a VM for the LC3 architecture

### What's a VM?

A VM is a piece of software that emulates the functioning of hardware other than the one it's running on. This is used to emulate different computer architectures to run programs compiled for a different architecture; most often to make a program made in one architecture compatible on other systems (such as the recent wave of ARM laptops, which use x86 emulation to run a wider variety of programs), or because the hardware that implemented the architecture being emulated isn't available anymore (for example, with old gaming systems).

### What's the LC3 achitecture?

The LC3 computer architecture is a simple computer architecture often used in university contexts to teach students the basics of computer architecture and assembly programming. This is because it's far simpler than modern [CISC](https://es.wikipedia.org/wiki/Complex_instruction_set_computing) architectures (like x86), while still being powerful enough and displaying most of the core concepts modern CPUs implement.

### How-to

You can run the VM with the command `make run path=[path to binary]` with the path to the lc3 binary you want to run.
The binaries folder has a few sample ones. You can run them with `make 2048` and `make rogue`.

You can run the tests with the command `make test`.

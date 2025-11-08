BINDIRS := bin bin/obj traces

# The aliases for used commands. Change the names if they
# are called differently in your setup.
AS := riscv64-linux-gnu-as 
LD := riscv64-linux-gnu-ld
SIMULATOR := ./risc-v-sim

LDFLAGS := -Ttext=0x80000000
LDFLAGS += -Tdata=0x80002000
LDFLAGS += -Trodata=0x80003000

SRCS = $(wildcard src/*.s)
OBJS = $(patsubst src/%,%.o,$(basename $(SRCS)))
ELFS = $(patsubst src/%,%.elf,$(basename $(SRCS)))
TRACES = $(patsubst src/%,%.json,$(basename $(SRCS)))

ALL_OBJS = $(addprefix bin/obj/, $(sort $(OBJS)))
ALL_ELFS = $(addprefix bin/, $(sort $(ELFS)))
ALL_TRACES = $(addprefix traces/, $(sort $(TRACES)))

$(ALL_OBJS) $(ALL_ELFS) $(ALL_TRACES): | $(BINDIRS)

$(BINDIRS):
	mkdir -p $@

bin/obj/%.o: src/%.s
	$(AS) $< -o $@

bin/%.elf: bin/obj/%.o
	$(LD) $(LDFLAGS) $< -o $@

traces/%.json: bin/%.elf
	$(SIMULATOR) --path bin/$*.elf --ticks $(file < cfg/$*) > $@

capture: $(ALL_TRACES)

build: $(ALL_ELFS)

.PHONY: debvars clean traces build all

clean:
	rm -rf bin traces

debvars:
	echo $(SRCS)
	echo $(OBJS)
	echo $(ELFS)
	echo $(TRACES)
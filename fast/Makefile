# MEV Protocol - C Hot Path
# PRODUCTION: Sub-microsecond latency optimizations

CC = gcc

# Core optimization flags
CFLAGS = -O3 -march=native -mtune=native -fPIC -Wall -Wextra
CFLAGS += -flto -ffast-math -funroll-loops -fno-exceptions
CFLAGS += -I./include

# SIMD: AVX2 + SSE4.2 for Intel/AMD
CFLAGS += -mavx2 -msse4.2 -mfma -mbmi2

# Cache optimization
CFLAGS += -falign-functions=64 -falign-loops=64
CFLAGS += -fprefetch-loop-arrays

# Inline aggressive
CFLAGS += -finline-functions -finline-limit=1000

# Link-time optimization
LDFLAGS = -flto -pthread

# Debug flags
DEBUG_FLAGS = -g -O0 -DDEBUG -fsanitize=address,undefined

# Source files
SRC_DIR = src
INC_DIR = include
OBJ_DIR = obj
LIB_DIR = lib

SOURCES = $(wildcard $(SRC_DIR)/*.c)
OBJECTS = $(SOURCES:$(SRC_DIR)/%.c=$(OBJ_DIR)/%.o)
HEADERS = $(wildcard $(INC_DIR)/*.h)

# Output library
STATIC_LIB = $(LIB_DIR)/libmev_fast.a
SHARED_LIB = $(LIB_DIR)/libmev_fast.so

# Test executable
TEST_SRC = test/test_all.c
TEST_BIN = test/test_runner

.PHONY: all clean test debug dirs

all: dirs $(STATIC_LIB) $(SHARED_LIB)

dirs:
	@mkdir -p $(OBJ_DIR) $(LIB_DIR)

# Compile object files
$(OBJ_DIR)/%.o: $(SRC_DIR)/%.c $(HEADERS)
	$(CC) $(CFLAGS) -c $< -o $@

# Static library
$(STATIC_LIB): $(OBJECTS)
	ar rcs $@ $^

# Shared library
$(SHARED_LIB): $(OBJECTS)
	$(CC) $(CFLAGS) -shared -o $@ $^

# Debug build
debug: CFLAGS = $(DEBUG_FLAGS) -I./include
debug: clean all

# Test build
test: all
	@mkdir -p test
	$(CC) $(CFLAGS) -o $(TEST_BIN) $(TEST_SRC) -L$(LIB_DIR) -lmev_fast
	./$(TEST_BIN)

# Benchmark
bench: all
	@echo "Running benchmarks..."
	$(CC) $(CFLAGS) -o bench/bench_runner bench/bench.c -L$(LIB_DIR) -lmev_fast
	./bench/bench_runner

clean:
	rm -rf $(OBJ_DIR) $(LIB_DIR) $(TEST_BIN)

# Install (Linux)
install: all
	install -d /usr/local/lib
	install -d /usr/local/include/mev
	install -m 644 $(STATIC_LIB) /usr/local/lib/
	install -m 755 $(SHARED_LIB) /usr/local/lib/
	install -m 644 $(HEADERS) /usr/local/include/mev/
	ldconfig

uninstall:
	rm -f /usr/local/lib/libmev_fast.*
	rm -rf /usr/local/include/mev

# Windows build (MinGW)
windows: CFLAGS += -D_WIN32
windows: all
	@echo "Windows build complete"

# Show compiler info
info:
	@echo "CC: $(CC)"
	@echo "CFLAGS: $(CFLAGS)"
	@echo "Sources: $(SOURCES)"
	@echo "Objects: $(OBJECTS)"

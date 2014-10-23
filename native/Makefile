ARCH := $(word 1, $(subst -, ,$(TARGET)))

ifeq ($(ARCH),i686)
CFLAGS += -m32 -fPIC
else
CFLAGS += -m64 -fPIC
endif

CFLAGS += -Wno-implicit-function-declaration

all:
	$(CC) $(CFLAGS) cfrunloop.c -c -o "$$OUT_DIR"/cfrunloop.o
	$(AR) crus "$$OUT_DIR"/libpackrnative.a "$$OUT_DIR"/cfrunloop.o
CFLAGS = -O2
LDLIBS = -lm

all: tt shapes chars

tt: Roboto.h

%: %.c
	$(CC) $(CFLAGS) $< -o $@ $(LDLIBS)

%.h: %.ttf Makefile
	python3 font_to_c.py $< 48 > $@

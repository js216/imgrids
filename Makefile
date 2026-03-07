FONT_SIZE = 36
CFLAGS = -O2 -DFONT_SIZE=$(FONT_SIZE)
LDLIBS = -lm

all: mono tt shapes chars
tt: Roboto.h
mono: RobotoMono-Regular.font

%: %.c Makefile
	$(CC) $(CFLAGS) $< -o $@ $(LDLIBS)

%.h: %.ttf Makefile
	python3 font_to_c.py $< $(FONT_SIZE) > $@

%.font: %.ttf Makefile
	python3 font_to_bin.py $< $(FONT_SIZE) > $@

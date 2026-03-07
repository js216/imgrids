FONT_SIZE ?= 12
CFLAGS = -O3 -DFONT_SIZE=${FONT_SIZE}
PROGS = shapes chars tt mono

all: $(PROGS)
tt: Roboto-Regular.font
mono: RobotoMono-Regular.font

%.font: %.ttf Makefile
	python3 font_to_bin.py $< $(FONT_SIZE) > $@

%: %.c Makefile
	$(CC) $(CFLAGS) $< -o $@

clean:
	rm -f $(PROGS) *.font

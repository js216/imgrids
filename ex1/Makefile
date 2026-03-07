FONT_SIZE ?= 36
CFLAGS = -O3 -DFONT_SIZE=${FONT_SIZE}
PROGS = shapes chars tt mono

all: $(PROGS)
tt: Roboto-Regular.font
mono: RobotoMono-Regular.font

%.font: ../fonts/%.ttf Makefile
	python3 ../fonts/font_to_bin.py $< $(FONT_SIZE) > $@

%: %.c Makefile
	$(CC) -I../fonts $(CFLAGS) $< -o $@

clean:
	rm -f $(PROGS) *.font

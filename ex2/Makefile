CFLAGS = -O3 -I../ex1/fonts
FONTS = RobotoMono-Regular-12.font RobotoMono-Regular-24.font \
        Roboto-Regular-12.font Roboto-Regular-24.font ../ex1/fonts/font8x8_basic.h

demo: shapes.o chars.o mono.o tt.o | $(FONTS)

%-12.font: ../ex1/fonts/%.ttf
	python3 ../ex1/fonts/font_to_bin.py $< 12 > $@

%-24.font: ../ex1/fonts/%.ttf
	python3 ../ex1/fonts/font_to_bin.py $< 24 > $@

%.o: %.c %.h
	$(CC) $(CFLAGS) -c $<

%: %.c
	$(CC) $(CFLAGS) $^ -o $@

clean:
	rm -f $(PROGS) *.font *.o
